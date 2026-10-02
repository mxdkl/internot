//! Profiles a world build: inclusive sample counts by function, top 50.
//! Run: cargo run --profile profiling -p internot_perf --example build_profile [pack]
use std::collections::HashMap;
use std::time::Instant;

use internot_society::{Params, World};

fn main() {
    let params = match std::env::args().nth(1) {
        Some(pack) => Params::load("worlds".as_ref(), &pack).expect("a pack in worlds/"),
        None => Params::prototype(),
    };
    let guard = pprof::ProfilerGuardBuilder::default().frequency(1000).build().unwrap();
    let t0 = Instant::now();
    let w = World::build(params, 42);
    println!("build {:?}, {} people", t0.elapsed(), w.population());
    let report = guard.report().build().unwrap();
    let mut inclusive: HashMap<String, usize> = HashMap::new();
    let mut total: usize = 0;
    for (frames, count) in report.data.iter() {
        let count = *count as usize;
        total += count;
        let mut seen = std::collections::HashSet::new();
        for f in frames.frames.iter().flatten() {
            let name = f.name();
            let parts = name.split("::").collect::<Vec<_>>();
            let short = parts[parts.len().saturating_sub(2)..].join("::");
            if seen.insert(short.clone()) {
                *inclusive.entry(short).or_default() += count;
            }
        }
    }
    let mut v: Vec<_> = inclusive.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    for (name, c) in v.iter().filter(|(n, _)| !n.contains("rayon") && !n.contains("closure")).take(45) {
        println!("{:5.1}%  {}", 100.0 * *c as f64 / total as f64, name);
    }
}
