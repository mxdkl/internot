//! Cost of a career and its parts, on random people (`MULT`, default 0.05).
use std::time::Instant;

use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use procedural_core::key::Key;

fn main() {
    let mult: f64 = std::env::var("MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, 42, mult);
    let xs: Vec<Pid> = (0..4000u64)
        .filter_map(|j| {
            let y = 1930 + (j % 70) as i32;
            let c = (j % m.cells() as u64) as u16;
            let n = m.cohort_n(c, y);
            (n > 0).then(|| Pid { cell: c, y, i: Key::from_seed(9).with(j).below(n) })
        })
        .collect();
    let _ = m.career(xs[0]);
    let time = |name: &str, f: &dyn Fn(Pid) -> usize| {
        let s = Instant::now();
        let h: usize = xs.iter().map(|&x| f(x)).sum();
        println!("{name}: {:.1} µs ({h})", s.elapsed().as_secs_f64() / xs.len() as f64 * 1e6);
    };
    time("career", &|x| m.career(x).len());
    time("career again", &|x| m.career(x).len());
    time("education_path", &|x| m.education_path(x).grades as usize);
    time("children", &|x| m.children(x).len());
    time("address_of at 30", &|x| m.address_of(x, m.birth(x) + 30 * 31_556_952).is_some() as usize);
    time("address_of at 31", &|x| m.address_of(x, m.birth(x) + 31 * 31_556_952).is_some() as usize);
}
