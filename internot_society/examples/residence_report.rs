//! Residence (L4) costs on the prototype world with a synthetic place tree
//! (`[areas, zones, counties, clusters, tracts]` per region from the arguments):
//! build time, first-touch closures, warm rosters, and address lookups.
//! Run: cargo run --release -p internot_society --example residence_report [areas zones counties clusters tracts]

use std::time::Instant;

use internot_society::residence::{Places, Residence, AREA, TRACT};
use internot_society::{Params, PersonId, World};
use procedural_core::key::Key;
use procedural_core::stream::{year_start, DAY};

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn pct(v: &mut [u64]) -> String {
    v.sort_unstable();
    let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
    format!(
        "p50 {} p90 {} p99 {} max {}",
        q(0.5),
        q(0.9),
        q(0.99),
        v[v.len() - 1]
    )
}

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|k| k.parse::<f64>().ok())
        .map_or(0.0, |k| k / 1024.0)
}

fn main() {
    let args: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let shape = if args.len() == 5 {
        [args[0], args[1], args[2], args[3], args[4]]
    } else {
        [10, 6, 3, 4, 8]
    };
    let t0 = Instant::now();
    let w = World::build(Params::prototype(), 42);
    println!(
        "world build {:?}, {} people ever, rss {:.0} MB",
        t0.elapsed(),
        w.population(),
        rss_mb()
    );
    let regions = w.ledger().params.region_count() as u16;
    let t0 = Instant::now();
    let places = Places::synthetic(regions, shape, 1840, 27, Key::from_seed(9));
    let r = Residence::build(&w, places);
    println!(
        "residence build {:?} ({} areas, {} tracts), rss {:.0} MB",
        t0.elapsed(),
        r.places().count(AREA),
        r.places().count(TRACT),
        rss_mb()
    );
    let t = year_start(2020) + 150 * DAY;
    let n = w.population();
    let mut x = 0x5eed_u64;

    // Cold and warm address lookups of people present in 2020.
    let ids: Vec<PersonId> = std::iter::from_fn(|| Some((xorshift(&mut x) % n) as PersonId))
        .filter(|&id| w.present_at(id, t))
        .take(5000)
        .collect();
    for pass in ["cold", "warm"] {
        let mut lat = Vec::with_capacity(ids.len());
        let mut sum = 0u64;
        for &id in &ids {
            let s = Instant::now();
            sum += r.address_of(&w, id, t).map_or(0, |p| p[TRACT] as u64);
            lat.push(s.elapsed().as_micros() as u64);
        }
        println!("address_of {pass} (µs): {} (checksum {sum})", pct(&mut lat));
    }
    println!("rss after lookups {:.0} MB", rss_mb());

    // First touch of a few areas, then warm tract rosters.
    let areas = r.places().count(AREA) as u32;
    for b in [0, areas / 4, areas / 2, 3 * areas / 4] {
        let s = Instant::now();
        let h = r.history(&w, b);
        let stretches: usize = h.members.iter().map(|m| m.stays.len()).sum();
        println!(
            "area {b}: first touch {:?}, {} units, {stretches} stretches, rss {:.0} MB",
            s.elapsed(),
            h.members.len(),
            rss_mb()
        );
        let mut lat = Vec::new();
        let mut households = 0;
        for tract in r.places().tracts_under(AREA, b) {
            let s = Instant::now();
            households += r.roster(&w, TRACT, tract, t).len();
            lat.push(s.elapsed().as_micros() as u64);
        }
        println!(
            "    tract rosters 2020 (µs): {}; {households} households",
            pct(&mut lat)
        );
    }
}
