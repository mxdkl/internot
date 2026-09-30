//! Profiling harness — drives the two hottest query paths in a tight loop
//! so samply / perf sample the inner inline-candidates without criterion's
//! measurement scaffolding polluting the stacks.
//!
//! Build:
//!   RUSTFLAGS="-C force-frame-pointers=yes" \
//!     cargo build --profile profiling --example profile_harness -p procedural_core
//!
//! Run (default 15 s):
//!   samply record ./target/profiling/examples/profile_harness find_filter_static
//!   samply record ./target/profiling/examples/profile_harness candidates_cosine
//!   samply record ./target/profiling/examples/profile_harness find_narrow
//!
//! Optional second arg is duration in whole seconds.
//!
//! Shares the exact fixture used by `benches/search.rs` so per-candidate
//! costs are directly comparable to the v0.13.0-benchmarks numbers in
//! `docs/benchmarks.md`.

#[path = "../benches/common/fixture.rs"]
mod fixture;

use chrono::{Duration, TimeZone, Utc};
use procedural_core::search::{stability, Op, StabilityMode};
use procedural_core::trajectory::{
    oscillate, oscillate_stability_radius, smooth, smooth_stability_radius,
};
use std::hint::black_box;
use std::time::{Duration as StdDuration, Instant};

fn mood(id: u64, t: chrono::DateTime<Utc>) -> f64 {
    let personality = ((id >> 22) & 0xF) as f64;
    let phase = personality * std::f64::consts::PI / 8.0;
    smooth(id, t, 0.5, Duration::days(3)) + 0.2 * oscillate(id, t, Duration::days(7), 0.3, phase)
}

fn mood_stability(id: u64, t: chrono::DateTime<Utc>, eps: f64) -> Duration {
    let personality = ((id >> 22) & 0xF) as f64;
    let phase = personality * std::f64::consts::PI / 8.0;
    stability::min_of(
        &[
            &|id, t, e| smooth_stability_radius(id, t, 0.5, Duration::days(3), e),
            &|id, t, e| oscillate_stability_radius(id, t, Duration::days(7), 0.3, phase, e),
        ],
        id,
        t,
        eps,
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let workload = args.next().unwrap_or_else(|| "find_filter_static".into());
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);

    let world = fixture::bench_world();
    let anchor = fixture::bench_reference_id(&world);
    let t = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = start + Duration::days(30);

    eprintln!("profile_harness: workload={workload}, duration={seconds}s, anchor=0x{anchor:016x}");
    let deadline = Instant::now() + StdDuration::from_secs(seconds);
    let mut iters: u64 = 0;

    match workload.as_str() {
        "find_filter_static" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let ids: Vec<u64> = people
                    .find()
                    .where_range("age", 25, 40)
                    .filter_static("even_entropy", |id| (id & 1) == 0)
                    .scan_budget(10_000)
                    .execute()
                    .collect();
                black_box(ids);
                iters += 1;
            }
        }
        "find_narrow" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let ids: Vec<u64> = people
                    .find()
                    .where_eq(black_box("family_id"), black_box(42))
                    .execute()
                    .take(10)
                    .collect();
                black_box(ids);
                iters += 1;
            }
        }
        "find_filter_temporal" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let ids: Vec<u64> = people
                    .find()
                    .where_range("age", 25, 40)
                    .at(t)
                    .filter_temporal("mood_gt_0_3", |id, t| mood(id, t) > 0.3)
                    .scan_budget(5_000)
                    .execute()
                    .collect();
                black_box(ids);
                iters += 1;
            }
        }
        "find_filter_exists_in_range" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let ids: Vec<u64> = people
                    .find()
                    .where_range("age", 25, 40)
                    .at_any(start, end)
                    .filter_exists_in_range(
                        "mood_lt_neg_0_3",
                        mood,
                        Op::Lt,
                        -0.3,
                        StabilityMode::Analytic(Box::new(mood_stability)),
                    )
                    .scan_budget(2_000)
                    .execute()
                    .collect();
                black_box(ids);
                iters += 1;
            }
        }
        "candidates_cosine" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let results = people
                    .candidates(black_box(anchor), "vibe")
                    .budget(5_000)
                    .take(10);
                black_box(results.hits);
                iters += 1;
            }
        }
        "candidates_euclidean_envelope" => {
            while Instant::now() < deadline {
                let people = world.space("people").unwrap();
                let results = people
                    .candidates(black_box(anchor), "lifestyle")
                    .envelope("locale_idx")
                    .budget(2_000)
                    .take(10);
                black_box(results.hits);
                iters += 1;
            }
        }
        other => {
            eprintln!(
                "unknown workload '{other}'. valid: find_filter_static, find_narrow, \
                 find_filter_temporal, find_filter_exists_in_range, candidates_cosine, \
                 candidates_euclidean_envelope"
            );
            std::process::exit(2);
        }
    }

    eprintln!("profile_harness: {iters} iterations in {seconds}s");
}
