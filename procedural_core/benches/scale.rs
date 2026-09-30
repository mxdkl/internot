//! Scale-validation benchmarks — 100M-equivalent scan budgets.
//!
//! These are slow and not meant for every `cargo bench` run. Invoke
//! explicitly: `cargo bench -p procedural_core --bench scale`.
//!
//! Primary purpose: validate that the framework stays approximately
//! linear in scan_budget, i.e., no O(n²) cliff between 10k and 100M.

#[path = "common/fixture.rs"]
mod fixture;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_scale_find(c: &mut Criterion) {
    let world = fixture::bench_world();
    let mut g = c.benchmark_group("scale_find");
    // Long-running — sample only 10 iterations to keep wall time reasonable.
    g.sample_size(10);

    g.bench_function("narrow_where_eq_drain_full", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            // family_id=42 with every other field free: 2^(64-16) = 2^48
            // candidate space. Budget caps the enumeration at 10M entries.
            let count: usize = people
                .find()
                .where_eq(black_box("family_id"), black_box(42))
                .scan_budget(10_000_000)
                .execute()
                .count();
            black_box(count)
        })
    });
    g.finish();
}

fn bench_scale_candidates(c: &mut Criterion) {
    let world = fixture::bench_world();
    let anchor = fixture::bench_reference_id(&world);
    let mut g = c.benchmark_group("scale_candidates");
    g.sample_size(10);

    g.bench_function("cosine_vibe_hash_budget_1m_take10", |b| {
        b.iter(|| {
            let people = world.space("people").unwrap();
            let results = people
                .candidates(black_box(anchor), "vibe")
                .budget(1_000_000)
                .take(10);
            black_box(results.hits)
        })
    });
    g.finish();
}

criterion_group!(benches, bench_scale_find, bench_scale_candidates);
criterion_main!(benches);
