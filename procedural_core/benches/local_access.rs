//! Criterion benchmarks for the Phase 0 local-access primitives.
//!
//! Run: `cargo bench -p procedural_core --bench local_access`
//!
//! Budgets are enforced by the perf gate (`perf/budgets.toml`); these
//! benchmarks give precise per-call numbers for tuning.

use chrono::{TimeZone, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use procedural_core::count::{CountTree, QuotaSpec, QuotaTree};
use procedural_core::graph::enumerate_events;
use procedural_core::key::Key;
use procedural_core::pairing::{Coupling, Pairing};
use procedural_core::partition::{Partition, SizeClasses, SplitTree};
use procedural_core::perm::{Bijection, FeistelPerm, SmallPerm};
use procedural_core::sample::{binomial, poisson, std_normal};
use procedural_core::stream::{hazard_time, poisson_buckets, PoissonTree, DAY};

fn bench_keys(c: &mut Criterion) {
    let k = Key::from_seed(1);
    let mut i = 0u64;
    c.bench_function("key_with", |b| {
        b.iter(|| {
            i = i.wrapping_add(1);
            black_box(k.with(i))
        })
    });
    c.bench_function("key_with3_unit", |b| {
        b.iter(|| {
            i = i.wrapping_add(1);
            black_box(k.with3(i, 7, 9).unit())
        })
    });
}

fn bench_perms(c: &mut Criterion) {
    let big = FeistelPerm::new(4_000_000_000, Key::from_seed(2));
    let mut x = 0u64;
    c.bench_function("feistel_fwd_4e9", |b| {
        b.iter(|| {
            x = (x + 7_919) % 4_000_000_000;
            black_box(big.fwd(x))
        })
    });
    c.bench_function("feistel_inv_4e9", |b| {
        b.iter(|| {
            x = (x + 7_919) % 4_000_000_000;
            black_box(big.inv(x))
        })
    });
    c.bench_function("feistel_build", |b| {
        b.iter(|| black_box(FeistelPerm::new(black_box(1_000_000), Key::from_seed(3))))
    });
    c.bench_function("small_perm_build_64", |b| {
        b.iter(|| black_box(SmallPerm::new(64, Key::from_seed(black_box(4)))))
    });
}

fn bench_matchings(c: &mut Criterion) {
    let p = Pairing::new(1_000_000_000, 800_000_000, Key::from_seed(5));
    let mut x = 0u64;
    c.bench_function("pairing_partner", |b| {
        b.iter(|| {
            x = (x + 104_729) % 1_000_000_000;
            black_box(p.partner(x))
        })
    });
    // 25 cohorts × gap kernel: a realistic union-table shape.
    let k = 25usize;
    let sizes = vec![200_000u64; k];
    let mut plan = vec![0u64; k * k];
    for a in 0..k {
        for d in 0..4usize {
            if a + d < k {
                let m = if d == 0 { 20_000 } else { 12_000 / d as u64 };
                plan[a * k + a + d] = m;
                plan[(a + d) * k + a] = m;
            }
        }
    }
    let cpl = Coupling::new(&sizes, &plan, Key::from_seed(6)).unwrap();
    let mut m = 0u64;
    c.bench_function("coupling_partner_25_blocks", |b| {
        b.iter(|| {
            m = (m + 7_919) % 200_000;
            black_box(cpl.partner(12, m))
        })
    });
}

fn bench_partitions(c: &mut Criterion) {
    let sc = SizeClasses::from_histogram(&[
        (1, 2_950_000),
        (2, 3_450_000),
        (3, 1_500_000),
        (4, 1_230_000),
        (5, 550_000),
        (6, 210_000),
        (7, 110_000),
    ]);
    let mut i = 0u64;
    let n = sc.len();
    c.bench_function("size_classes_group_of", |b| {
        b.iter(|| {
            i = (i + 7_919) % n;
            black_box(sc.group_of(i))
        })
    });
    let st = SplitTree::new(10_000_000, 1_000, 5_000, Key::from_seed(7));
    c.bench_function("split_tree_leaf_of_1e7", |b| {
        b.iter(|| {
            i = (i + 7_919) % 10_000_000;
            black_box(st.leaf_of(i))
        })
    });
    let ct = CountTree::new(Key::from_seed(8), 20, 5_000_000, |lo, hi| (hi - lo) as f64);
    c.bench_function("count_tree_count_before_20lvl", |b| {
        b.iter(|| {
            i = (i + 7_919) % (1 << 20);
            black_box(ct.count_before(i))
        })
    });
    let cells: Vec<QuotaSpec> = (0..2)
        .map(|sex| {
            QuotaSpec::node(
                sex,
                (0..40)
                    .map(|u| {
                        QuotaSpec::node(
                            u,
                            (0..12)
                                .map(|p| QuotaSpec::leaf(p, 97 + (u * 13 + p) as u64 % 50))
                                .collect(),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    let q = QuotaTree::new(&cells).unwrap();
    let total = q.total();
    c.bench_function("quota_locate_3lvl", |b| {
        b.iter(|| {
            i = (i + 7_919) % total;
            black_box(q.locate(i))
        })
    });
}

fn bench_samplers(c: &mut Criterion) {
    let k = Key::from_seed(9);
    let mut i = 0u64;
    for (name, lam) in [
        ("poisson_lambda_3", 3.0),
        ("poisson_lambda_50", 50.0),
        ("poisson_lambda_1e5", 1e5),
    ] {
        c.bench_function(name, |b| {
            b.iter(|| {
                i += 1;
                black_box(poisson(k.with(i), lam))
            })
        });
    }
    c.bench_function("binomial_inversion_n100_p0.1", |b| {
        b.iter(|| {
            i += 1;
            black_box(binomial(k.with(i), 100, 0.1))
        })
    });
    c.bench_function("binomial_btpe_n1e6_p0.3", |b| {
        b.iter(|| {
            i += 1;
            black_box(binomial(k.with(i), 1_000_000, 0.3))
        })
    });
    c.bench_function("std_normal", |b| {
        b.iter(|| {
            i += 1;
            black_box(std_normal(k.with(i)))
        })
    });
}

fn bench_streams(c: &mut Criterion) {
    let k = Key::from_seed(10);
    let mut out = Vec::with_capacity(10_000);
    c.bench_function("poisson_buckets_30d_rate5", |b| {
        b.iter(|| {
            out.clear();
            poisson_buckets(k, DAY, |_| 5.0, 80_000 * DAY, 80_030 * DAY, &mut out);
            black_box(out.len())
        })
    });
    let tree = PoissonTree::new(k, 0, DAY, 18, |a: i64, b: i64| {
        (b - a) as f64 / (400.0 * DAY as f64)
    });
    let mut t = 0i64;
    c.bench_function("poisson_tree_count_before_18lvl", |b| {
        b.iter(|| {
            t = (t + 7_919 * DAY) % ((1 << 18) * DAY);
            black_box(tree.count_before(t))
        })
    });
    c.bench_function("hazard_time_piecewise_annual_100y", |b| {
        b.iter(|| {
            // Annual hazard pieces over 100 years, never firing: worst case.
            black_box(hazard_time(1e9, 0, 100 * 365 * DAY, |t| {
                (1e-12, (t / (365 * DAY) + 1) * 365 * DAY)
            }))
        })
    });
    let t0 = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0).unwrap();
    let t1 = Utc.with_ymd_and_hms(2050, 1, 1, 0, 0, 0).unwrap();
    c.bench_function("enumerate_events_100y_lambda0.01", |b| {
        b.iter(|| black_box(enumerate_events(7, 42, "mail", |_| 0.01, t0, t1).len()))
    });
}

criterion_group!(
    benches,
    bench_keys,
    bench_perms,
    bench_matchings,
    bench_partitions,
    bench_samplers,
    bench_streams
);
criterion_main!(benches);
