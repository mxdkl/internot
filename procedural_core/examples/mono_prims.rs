//! Speed of the monotone-world primitives (`thinking/claude/005`):
//! `AffinePerm` counts and selects, `ExactInterleave` locate, count and
//! select.
use std::hint::black_box;
use std::time::Instant;

use procedural_core::key::Key;
use procedural_core::lattice::ExactInterleave;
use procedural_core::perm::{AffinePerm, Bijection};

fn time(name: &str, reps: u64, mut f: impl FnMut(u64) -> u64) {
    let s = Instant::now();
    let mut h = 0u64;
    for i in 0..reps {
        h = h.wrapping_add(f(i));
    }
    black_box(h);
    println!("{name:>40}: {:.0} ns", s.elapsed().as_nanos() as f64 / reps as f64);
}

fn main() {
    for n in [1_000u64, 1_000_000, 1 << 32, 1 << 40] {
        time(&format!("affine new n≈2^{:.0}", (n as f64).log2()), 200_000, |i| AffinePerm::new(n + i % 1000, Key::from_seed(i)).parts().0);
    }
    for n in [1_000u64, 1_000_000, 1 << 32, 1 << 40] {
        let p = AffinePerm::new(n, Key::from_seed(9));
        let m = (n / 7919).max(1);
        time(&format!("affine fwd n=2^{:.0}", (n as f64).log2()), 1_000_000, |i| p.fwd((i * m) % n));
        time(&format!("affine inv n=2^{:.0}", (n as f64).log2()), 1_000_000, |i| p.inv((i * m) % n));
        time(&format!("affine count n=2^{:.0}", (n as f64).log2()), 200_000, |i| {
            let lo = (i * m) % n;
            p.count(lo / 2, lo, n / 3, n / 3 + lo / 5)
        });
        time(&format!("affine select n=2^{:.0}", (n as f64).log2()), 20_000, |i| p.select(0, n / 2, n, (i * m) % (n / 3)).unwrap_or(0));
    }
    // An exact interleave of 50 husband cohorts (the age-gap kernel's
    // support) over a wife cohort.
    for total in [1_000_000u64, 1 << 32] {
        let sizes: Vec<u64> = (0..50).map(|c| ((-((c as f64 - 21.0).powi(2)) / 30.0).exp() * total as f64 / 7.0) as u64).collect();
        let mut pre = vec![0u64];
        for s in &sizes {
            pre.push(pre.last().unwrap() + s);
        }
        let k = *pre.last().unwrap();
        let e = ExactInterleave { categories: 50, prefix: |i: usize| pre[i], key: |node: u64| Key::from_seed(4).with(node).below(u64::MAX) };
        let m = (k / 7919).max(1);
        let tag = format!("k=2^{:.0}", (k as f64).log2());
        time(&format!("interleave locate {tag}"), 200_000, |i| e.locate((i * m) % k).1);
        time(&format!("interleave count {tag}"), 200_000, |i| e.count((i % 50) as usize, (i * m) % k));
        time(&format!("interleave select {tag}"), 200_000, |i| {
            let c = 15 + (i % 12) as usize;
            e.select(c, (i * 7) % sizes[c].max(1))
        });
    }
}
