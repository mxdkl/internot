//! Times a world build and reports its size: blocks, union cells and peak
//! memory. Run: cargo run --release -p internot_society --example build_time [pack]
//! (a pack in `worlds/`, or `$WORLDS`; default: the built-in `us`).
use std::time::Instant;

use internot_society::{Params, World};

fn rss_mb(field: &str) -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|k| k.parse::<f64>().ok())
        .map_or(0.0, |k| k / 1024.0)
}

fn main() {
    let params = match std::env::args().nth(1) {
        Some(pack) => {
            let root = std::env::var("WORLDS").unwrap_or_else(|_| "worlds".into());
            Params::load(root.as_ref(), &pack).expect("a pack in worlds/ (or $WORLDS)")
        }
        None => Params::prototype(),
    };
    let regions = params.region_count();
    let t0 = Instant::now();
    let w = World::build(params, 42);
    let l = w.ledger();
    let cells = w.union_cell_count();
    println!(
        "{regions} regions: build {:?}, {} blocks, {cells} union cells, {} people; rss {:.0} MB, peak {:.0} MB",
        t0.elapsed(),
        l.blocks.len(),
        w.population(),
        rss_mb("VmRSS:"),
        rss_mb("VmHWM:")
    );
    let report = w.memory_report();
    let total: usize = report.iter().map(|e| e.1).sum();
    let allocs: usize = report.iter().map(|e| e.2).sum();
    println!("heap by component: {:.0} MB in {allocs} allocations", total as f64 / 1e6);
    for (name, bytes, n) in report {
        println!("  {:>8.1} MB  {n:>10} allocs  {name}", bytes as f64 / 1e6);
    }
}
