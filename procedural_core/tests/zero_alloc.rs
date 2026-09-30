//! Gate: hot-path primitives make zero heap allocations.
//!
//! Spec §16.2 requires allocation-free hot paths (keys, permutations,
//! matchings, partitions, count and quota lookups, samplers, event draws).
//! This test binary installs a counting global allocator and asserts that
//! each hot path allocates nothing once its structure is built.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use procedural_core::count::{CountTree, QuotaSpec, QuotaTree};
use procedural_core::key::Key;
use procedural_core::pairing::{Coupling, Pairing};
use procedural_core::partition::{Partition, SizeClasses, SplitTree};
use procedural_core::perm::{Bijection, CompactPerm, FeistelPerm, GrowablePerm, SmallPerm};
use procedural_core::sample::{binomial, lognormal, lomax_lifetime, poisson, std_normal};
use procedural_core::stream::{hazard_time, poisson_buckets, PoissonTree, DAY};

struct CountingAlloc;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Allocations made by `f` on this thread.
fn allocs<R>(f: impl FnOnce() -> R) -> (u64, R) {
    let before = ALLOCS.with(|c| c.get());
    let r = std::hint::black_box(f());
    (ALLOCS.with(|c| c.get()) - before, r)
}

fn assert_zero<R>(name: &str, f: impl FnOnce() -> R) {
    let (n, _) = allocs(f);
    assert_eq!(n, 0, "{name} allocated {n} times");
}

#[test]
fn keys_and_samplers_do_not_allocate() {
    let k = Key::from_seed(1);
    assert_zero("Key derivation", || {
        k.with(1).with2(2, 3).with3(4, 5, 6).unit()
    });
    assert_zero("poisson small", || {
        (0..1000).map(|i| poisson(k.with(i), 3.5)).sum::<u64>()
    });
    assert_zero("poisson large", || {
        (0..1000).map(|i| poisson(k.with(i), 5000.0)).sum::<u64>()
    });
    assert_zero("binomial inversion", || {
        (0..1000)
            .map(|i| binomial(k.with(i), 100, 0.1))
            .sum::<u64>()
    });
    assert_zero("binomial BTPE", || {
        (0..1000)
            .map(|i| binomial(k.with(i), 1_000_000, 0.3))
            .sum::<u64>()
    });
    assert_zero("normal/lognormal/lomax", || {
        (0..1000)
            .map(|i| {
                std_normal(k.with(i))
                    + lognormal(k.with(i), 0.0, 1.0)
                    + lomax_lifetime(k.with(i), -0.7)
            })
            .sum::<f64>()
    });
}

#[test]
fn permutations_do_not_allocate() {
    assert_zero("FeistelPerm build + use", || {
        let p = FeistelPerm::new(4_000_000_000, Key::from_seed(2));
        (0..1000).map(|x| p.inv(p.fwd(x))).sum::<u64>()
    });
    assert_zero("GrowablePerm", || {
        let g = GrowablePerm::new(1_000_000, Key::from_seed(3));
        (0..1000)
            .map(|x| g.inv(g.fwd(x, 999_000), 999_000))
            .sum::<u64>()
    });
    assert_zero("CompactPerm build + use", || {
        let p = CompactPerm::new(4_000_000_000, Key::from_seed(3));
        (0..1000).map(|x| p.inv(p.fwd(x))).sum::<u64>()
    });
    assert_zero("SmallPerm build + use", || {
        let s = SmallPerm::new(64, Key::from_seed(4));
        (0..64).map(|x| s.inv(s.fwd(x))).sum::<u64>()
    });
}

#[test]
fn matchings_do_not_allocate_after_build() {
    let p = Pairing::new(1_000_000, 800_000, Key::from_seed(5));
    assert_zero("Pairing", || {
        (0..1000).filter_map(|x| p.partner(x)).sum::<u64>()
    });
    let c = Coupling::new(
        &[1000, 1200, 800],
        &[100, 200, 50, 200, 150, 300, 50, 300, 100],
        Key::from_seed(6),
    )
    .unwrap();
    assert_zero("Coupling", || {
        (0..800)
            .filter_map(|m| c.partner(2, m).map(|(_, x)| x).zip(c.couple_id(2, m)))
            .count()
    });
}

#[test]
fn partitions_and_counts_do_not_allocate_after_build() {
    let sc = SizeClasses::from_histogram(&[(1, 300), (2, 350), (3, 150), (4, 120), (7, 11)]);
    assert_zero("SizeClasses", || {
        (0..sc.len())
            .map(|i| sc.group_of(i).0 + sc.range_of(sc.group_of(i).0).start)
            .sum::<u64>()
    });
    let st = SplitTree::new(1_000_000, 20, 60, Key::from_seed(7));
    assert_zero("SplitTree::leaf_of", || {
        (0..1000).map(|i| st.leaf_of(i * 997).start).sum::<u64>()
    });
    let ct = CountTree::new(Key::from_seed(8), 20, 1_000_000, |lo, hi| (hi - lo) as f64);
    assert_zero("CountTree", || {
        (0..200)
            .map(|i| ct.count_before(i * 5000) + ct.select(i * 4000).0)
            .sum::<u64>()
    });
    let q = QuotaTree::new(&[
        QuotaSpec::node(0, vec![QuotaSpec::leaf(1, 500), QuotaSpec::leaf(2, 300)]),
        QuotaSpec::node(1, vec![QuotaSpec::leaf(3, 700), QuotaSpec::leaf(4, 100)]),
    ])
    .unwrap();
    assert_zero("QuotaTree::locate", || {
        (0..q.total()).map(|r| q.locate(r).offset).sum::<u64>()
    });
}

#[test]
fn event_streams_do_not_allocate_beyond_their_output() {
    let k = Key::from_seed(9);
    let mut out = Vec::with_capacity(100_000);
    assert_zero("poisson_buckets into reserved output", || {
        poisson_buckets(k, DAY, |_| 2.0, 0, 10_000 * DAY, &mut out);
        out.len()
    });
    let tree = PoissonTree::new(k, 0, DAY, 18, |a: i64, b: i64| {
        (b - a) as f64 / (30.0 * DAY as f64)
    });
    assert_zero("PoissonTree::count_before / last_before", || {
        (0..200)
            .map(|i| {
                tree.count_before(i * 1000 * DAY)
                    + tree.last_before(i * 1000 * DAY).map_or(0, |e| e.t as u64)
            })
            .sum::<u64>()
    });
    assert_zero("hazard_time", || {
        (0..1000)
            .filter_map(|i| {
                hazard_time(k.with(i).unit() * 5.0, 0, 100_000 * DAY, |_| {
                    (1.0 / DAY as f64, i64::MAX)
                })
            })
            .count()
    });
}
