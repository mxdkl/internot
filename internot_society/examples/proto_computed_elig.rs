//! Prototype (research/2026-10-04-workplace-rosters.md §3): what computed
//! classes would cost a mother lookup. Recomputes a year's eligible mothers
//! by (kind, age) from class sizes and one death threshold per mother
//! cohort, against the stored rows, and times both against `mother`.
use std::time::Instant;

use internot_society::mono::{Mono, Pid};
use internot_society::Params;
use procedural_core::key::Key;

fn main() {
    let p = Params::embedded("us").unwrap();
    let m = Mono::new(&p, 42, 1.0);
    let ys: Vec<(u16, i32)> = (0..20_000u64).map(|j| ((j % m.cells() as u64) as u16, 1900 + Key::from_seed(3).with(j).below(120) as i32)).collect();
    let mut bad = 0;
    for &(c, y) in ys.iter().take(2000) {
        if m.eligible_from_scratch(c, y) != m.eligible_stored(c, y) {
            bad += 1;
        }
    }
    println!("recomputed rows differing from stored: {bad} of 2000");
    let s = Instant::now();
    let mut h = 0u64;
    for &(c, y) in &ys {
        h = h.wrapping_add(m.eligible_from_scratch(c, y).iter().sum::<u64>());
    }
    let scratch = s.elapsed().as_secs_f64() / ys.len() as f64 * 1e9;
    let s = Instant::now();
    for &(c, y) in &ys {
        h = h.wrapping_add(m.eligible_stored(c, y).iter().sum::<u64>());
    }
    let stored = s.elapsed().as_secs_f64() / ys.len() as f64 * 1e9;
    let xs: Vec<Pid> = (0..200_000u64)
        .filter_map(|j| {
            let c = (j % m.cells() as u64) as u16;
            let y = 1900 + Key::from_seed(4).with(j).below(120) as i32;
            let n = m.cohort_n(c, y);
            (n > 0).then(|| Pid { cell: c, y, i: Key::from_seed(5).with(j).below(n) })
        })
        .collect();
    let s = Instant::now();
    for &x in &xs {
        h = h.wrapping_add(m.mother(x).map_or(0, |q| q.i));
    }
    let mother = s.elapsed().as_secs_f64() / xs.len() as f64 * 1e9;
    println!("a year's eligible rows (62 blocks): recomputed {scratch:.0} ns, read {stored:.0} ns; mother lookup {mother:.0} ns ({h})");
}
