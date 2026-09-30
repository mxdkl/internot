//! Single-call latency of the kinship lookups over uniform ids, plus a
//! checksum of every answer. It is meant for A/B runs (`perf/ab.sh`), which
//! resolve ~5% differences that separate gate runs cannot. Two builds whose
//! checksums differ do not give the same answers.
//!
//! ```sh
//! cargo run --release -p internot_society --example lookup_timing -- [ids]
//! ```
//!
//! Prints `query p50/p90/p99` in nanoseconds for each query, then `sum`.

use internot_society::{Params, PersonId, World};
use std::time::Instant;

fn main() {
    let n_ids: usize = std::env::args()
        .nth(1)
        .map_or(200_000, |s| s.parse().expect("an id count"));
    let w = World::build(Params::prototype(), 42);
    let n = w.population();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let ids: Vec<PersonId> = (0..n_ids)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n) as PersonId
        })
        .collect();
    let mut sum = 0u64;
    let pct = |v: &[u64], q: f64| v[((v.len() - 1) as f64 * q) as usize];
    let mut line = String::new();
    for name in ["death", "mother", "father", "union", "children", "siblings"] {
        let mut lat = Vec::with_capacity(ids.len());
        for &id in &ids {
            let t = Instant::now();
            let v = match name {
                "death" => w.death(id) as u64,
                "mother" => w.mother(id).map_or(0, u64::from),
                "father" => w.father(id).map_or(0, u64::from),
                "union" => w.union(id).map_or(0, |u| u.partner as u64),
                "children" => w.children(id).iter().map(|&c| c as u64).sum(),
                _ => w.siblings(id).iter().map(|&c| c as u64).sum(),
            };
            lat.push(t.elapsed().as_nanos() as u64);
            sum = sum.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
        }
        lat.sort_unstable();
        line += &format!(
            "{name} {}/{}/{}  ",
            pct(&lat, 0.5),
            pct(&lat, 0.9),
            pct(&lat, 0.99)
        );
    }
    println!("{line}sum {sum:016x}");
}
