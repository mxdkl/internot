//! Criterion benchmarks for Layer 0 primitives.
//!
//! Run: `cargo bench -p procedural_core`
//!
//! Target costs:
//! - hash primitives: < 100 ns per call
//! - trajectory primitives: < 200 ns per call
//! - samplers: < 100 ns per call
//! - edge functions: < 100 ns per call
//! - BitLayout extract/compose: < 50 ns per call
//! - stability-radius: < 500 ns per call (includes multiple derivative evaluations)

use chrono::{DateTime, Duration, TimeZone, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use procedural_core::bits::BitLayout;
use procedural_core::edge::{block, cosine, geometric, hyperbolic};
use procedural_core::hash::{hash_float, hash_gaussian, hash_int, hash_vec};
use procedural_core::sampler::{categorical, exponential, lognormal, pareto};
use procedural_core::trajectory::{
    oscillate, oscillate_stability_radius, smooth, smooth_stability_radius,
    stability_radius_quadratic, step, step_stability_radius,
};

fn bench_hash(c: &mut Criterion) {
    c.bench_function("hash_float_u64", |b| {
        b.iter(|| hash_float(black_box(42u64), black_box("age")))
    });
    c.bench_function("hash_float_u128", |b| {
        b.iter(|| hash_float(black_box(42u128), black_box("age")))
    });
    c.bench_function("hash_int_u64", |b| {
        b.iter(|| hash_int(black_box(42u64), black_box("locale"), black_box(256)))
    });
    c.bench_function("hash_vec_u64_5dims", |b| {
        b.iter(|| hash_vec(black_box(42u64), black_box("personality"), 5))
    });
    c.bench_function("hash_gaussian_u64", |b| {
        b.iter(|| hash_gaussian(black_box(42u64), black_box("z")))
    });
}

fn bench_bits(c: &mut Criterion) {
    let layout =
        BitLayout::<u64>::new(vec![("gen", 4), ("fam", 28), ("sib", 4), ("ent", 28)]).unwrap();
    let id = layout.compose(&[
        ("gen", 3),
        ("fam", 0x1234567),
        ("sib", 7),
        ("ent", 0x89ABCDE),
    ]);

    c.bench_function("bitlayout_u64_extract", |b| {
        b.iter(|| layout.extract(black_box(id), black_box("fam")))
    });
    c.bench_function("bitlayout_u64_compose", |b| {
        b.iter(|| {
            layout.compose(&[
                ("gen", 3),
                ("fam", 0x1234567),
                ("sib", 7),
                ("ent", 0x89ABCDE),
            ])
        })
    });

    let layout128 =
        BitLayout::<u128>::new(vec![("a", 16), ("b", 48), ("c", 16), ("d", 48)]).unwrap();
    let id128: u128 = layout128.compose(&[
        ("a", 0xABCD),
        ("b", 0x0000_FFFF_FFFF_FFFF),
        ("c", 0x1234),
        ("d", 0x0000_0000_DEAD_BEEF),
    ]);

    c.bench_function("bitlayout_u128_extract", |b| {
        b.iter(|| layout128.extract(black_box(id128), black_box("b")))
    });
}

fn bench_trajectory(c: &mut Criterion) {
    let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();

    c.bench_function("smooth", |b| {
        b.iter(|| smooth(black_box(42u64), black_box(t), 1.0, Duration::days(30)))
    });
    c.bench_function("oscillate", |b| {
        b.iter(|| oscillate(black_box(42u64), black_box(t), Duration::days(7), 0.5, 0.0))
    });

    let events = vec![
        Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
    ];
    let magnitudes = vec![5.0, 10.0];
    c.bench_function("step", |b| {
        b.iter(|| step(black_box(42u64), black_box(t), &events, &magnitudes))
    });
}

fn bench_stability(c: &mut Criterion) {
    let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();

    c.bench_function("stability_radius_quadratic_linear", |b| {
        b.iter(|| stability_radius_quadratic(black_box(10.0), 0.0, black_box(1.0)))
    });
    c.bench_function("stability_radius_quadratic_full", |b| {
        b.iter(|| stability_radius_quadratic(black_box(5.0), black_box(2.0), black_box(0.1)))
    });
    c.bench_function("smooth_stability_radius", |b| {
        b.iter(|| {
            smooth_stability_radius(
                black_box(42u64),
                black_box(t),
                1.0,
                Duration::days(30),
                0.05,
            )
        })
    });
    c.bench_function("oscillate_stability_radius", |b| {
        b.iter(|| {
            oscillate_stability_radius(
                black_box(42u64),
                black_box(t),
                Duration::days(7),
                1.0,
                0.3,
                0.05,
            )
        })
    });

    let events = vec![
        Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2035, 1, 1, 0, 0, 0).unwrap(),
    ];
    let magnitudes = vec![5.0, 10.0];
    c.bench_function("step_stability_radius", |b| {
        b.iter(|| step_stability_radius(black_box(42u64), black_box(t), &events, &magnitudes, 1.0))
    });
}

fn bench_sampler(c: &mut Criterion) {
    c.bench_function("pareto", |b| {
        b.iter(|| pareto(black_box(42u64), "size", 1.5, 1.0))
    });
    c.bench_function("lognormal", |b| {
        b.iter(|| lognormal(black_box(42u64), "x", 0.0, 1.0))
    });
    c.bench_function("exponential", |b| {
        b.iter(|| exponential(black_box(42u64), "x", 2.0))
    });
    let weights = [0.1, 0.6, 0.3];
    c.bench_function("categorical", |b| {
        b.iter(|| categorical(black_box(42u64), "x", &weights))
    });
}

fn bench_edge(c: &mut Criterion) {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b_vec = [1.1, 2.1, 3.1, 4.1, 5.1];

    c.bench_function("geometric_5d", |b| {
        b.iter(|| geometric(black_box(&a), black_box(&b_vec), 1.0, true))
    });
    c.bench_function("cosine_5d", |b| {
        b.iter(|| cosine(black_box(&a), black_box(&b_vec), 0.0))
    });
    c.bench_function("hyperbolic", |b| {
        b.iter(|| hyperbolic(black_box((0.5, 0.0)), black_box((0.5, 1.0)), 5.0, 0.3))
    });
    c.bench_function("block", |b| {
        b.iter(|| block(black_box(5u64), black_box(7u64), 0.8, 0.1))
    });
}

fn bench_search_primitives(c: &mut Criterion) {
    use procedural_core::search::{stability, Op};

    c.bench_function("op_check_gt", |b| {
        b.iter(|| black_box(Op::Gt).check(black_box(1.5), black_box(1.0)))
    });
    c.bench_function("op_check_eq_f64", |b| {
        b.iter(|| black_box(Op::Eq).check(black_box(1.5), black_box(1.5)))
    });

    c.bench_function("stability_min_of_2_components", |b| {
        let t = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let f_a = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(5);
        let f_b = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(3);
        b.iter(|| {
            stability::min_of(
                &[&f_a, &f_b],
                black_box(42u64),
                black_box(t),
                black_box(1.0),
            )
        })
    });
    c.bench_function("stability_min_of_3_components", |b| {
        let t = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let f_a = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(5);
        let f_b = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(3);
        let f_c = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(7);
        b.iter(|| {
            stability::min_of(
                &[&f_a, &f_b, &f_c],
                black_box(42u64),
                black_box(t),
                black_box(1.0),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_hash,
    bench_bits,
    bench_trajectory,
    bench_stability,
    bench_sampler,
    bench_edge,
    bench_search_primitives,
);
criterion_main!(benches);
