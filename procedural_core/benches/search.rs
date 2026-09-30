//! Query-tier benchmarks — realistic find() and candidates() workloads
//! at 1M-equivalent scale via `scan_budget(N)`.
//!
//! All benches use the shared fixture from `common/fixture.rs` (declared
//! via `#[path]` so the module is shared with scale.rs).
//!
//! Run: `cargo bench -p procedural_core --bench search`.

#[path = "common/fixture.rs"]
mod fixture;

use chrono::{Duration, TimeZone, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use procedural_core::search::{stability, Op, StabilityMode};
use procedural_core::trajectory::{
    oscillate, oscillate_stability_radius, smooth, smooth_stability_radius,
};

/// Simple mood closure — sum of smooth + oscillate. Mirrors the demo but
/// uses constants so there's no table dependency.
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

fn bench_find(c: &mut Criterion) {
    let world = fixture::bench_world();
    let mut g = c.benchmark_group("find");

    g.bench_function("narrow_where_eq_take10", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let ids: Vec<u64> = people
                .find()
                .where_eq(black_box("family_id"), black_box(42))
                .execute()
                .take(10)
                .collect();
            black_box(ids)
        })
    });

    g.bench_function("multi_predicate_take10", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let ids: Vec<u64> = people
                .find()
                .where_eq("personality", 3)
                .where_eq("chronotype", 0)
                .where_range("age", 25, 40)
                .execute()
                .take(10)
                .collect();
            black_box(ids)
        })
    });

    g.bench_function("filter_static_budget_10k", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let ids: Vec<u64> = people
                .find()
                .where_range("age", 25, 40)
                .filter_static("even_entropy", |id| (id & 1) == 0)
                .scan_budget(10_000)
                .execute()
                .collect();
            black_box(ids)
        })
    });

    let t = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
    g.bench_function("filter_temporal_budget_5k", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let ids: Vec<u64> = people
                .find()
                .where_range("age", 25, 40)
                .at(t)
                .filter_temporal("mood_gt_0_3", |id, t| mood(id, t) > 0.3)
                .scan_budget(5_000)
                .execute()
                .collect();
            black_box(ids)
        })
    });

    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = start + Duration::days(30);
    g.bench_function("filter_exists_in_range_budget_2k", |b| {
        b.iter(|| {
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
            black_box(ids)
        })
    });

    g.finish();
}

fn bench_candidates(c: &mut Criterion) {
    let world = fixture::bench_world();
    let anchor = fixture::bench_reference_id(&world);
    let mut g = c.benchmark_group("candidates");

    g.bench_function("cosine_vibe_hash_budget_5k_take10", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let results = people
                .candidates(black_box(anchor), "vibe")
                .budget(5_000)
                .take(10);
            black_box(results.hits)
        })
    });

    g.bench_function("euclidean_lifestyle_envelope_budget_2k_take10", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let results = people
                .candidates(black_box(anchor), "lifestyle")
                .envelope("locale_idx")
                .budget(2_000)
                .take(10);
            black_box(results.hits)
        })
    });

    g.finish();
}

criterion_group!(benches, bench_find, bench_candidates);
criterion_main!(benches);
