//! Profiles a residence closure (first touch of one area) on the tiny
//! world: inclusive sample counts by function, top 40.
//! Run: cargo run --profile profiling -p internot_perf --example residence_profile

use std::collections::HashMap;
use std::time::Instant;

use internot_society::residence::{Places, Residence};
use internot_society::{Params, World};
use procedural_core::key::Key;

fn main() {
    let proto = std::env::args().any(|a| a == "prototype");
    let w = if proto {
        World::build(Params::prototype(), 42)
    } else {
        World::build(Params::tiny(), 7)
    };
    let regions = w.ledger().params.region_count() as u16;
    let places = if proto {
        Places::synthetic(regions, [25, 6, 3, 4, 8], 1840, 27, Key::from_seed(9))
    } else {
        Places::synthetic(regions, [2, 3, 2, 2, 3], 1900, 10, Key::from_seed(107))
    };
    let r = Residence::build(&w, places);
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(2000)
        .build()
        .unwrap();
    let t0 = Instant::now();
    let h = r.history(&w, 0);
    println!("area 0: {} units in {:?}", h.members.len(), t0.elapsed());
    let report = guard.report().build().unwrap();
    let mut inclusive: HashMap<String, usize> = HashMap::new();
    let mut total: usize = 0;
    for (frames, count) in report.data.iter() {
        let count = *count as usize;
        total += count;
        let mut seen = std::collections::HashSet::new();
        for f in frames.frames.iter().flatten() {
            let name = f.name();
            let short = name.split("::").collect::<Vec<_>>();
            let short = short[short.len().saturating_sub(2)..].join("::");
            if seen.insert(short.clone()) {
                *inclusive.entry(short).or_default() += count;
            }
        }
    }
    let mut v: Vec<_> = inclusive.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    for (name, c) in v.iter().take(45) {
        println!("{:5.1}%  {}", 100.0 * *c as f64 / total as f64, name);
    }
}
