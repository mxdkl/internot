//! Cohort smoke test. Validates contains/sample on a cohort returned
//! by `Query::into_cohort()`.

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::word::{BitWord, U512};
use procedural_core::world::World;

#[test]
fn cohort_contains_and_sample_round_trip() {
    let layout = BitLayout::<U512>::new(vec![
        ("member_idx", 12),
        ("workplace_seed", 8),
        ("city_idx", 6),
        ("industry_idx", 6),
    ])
    .expect("32-bit layout fits in U512");
    let mut space = Space::<U512>::new("smoke", layout);
    space
        .indexable_attribute::<u32, _>("industry_idx", "industry_idx", |v| v as u32)
        .unwrap();
    let mut world = World::<U512>::new();
    world.register(space).unwrap();
    let s = world.space("smoke").unwrap();

    let cohort = s.find().where_eq("industry_idx", 5).into_cohort();

    // 10 samples should all land in industry=5.
    let samples = cohort.sample(10);
    assert_eq!(samples.len(), 10);
    for id in &samples {
        assert_eq!(id.extract_bits(26, 6), 5, "industry_idx mismatch in sample");
    }

    // Each sampled id should be `contains()`-true.
    for id in &samples {
        assert!(cohort.contains(*id), "sample not contained: {id}");
    }

    // An id with industry=6 should NOT be contained.
    let mismatch = U512::default().insert_bits(26, 6, 6);
    assert!(!cohort.contains(mismatch));
}

#[test]
fn cohort_sample_is_deterministic() {
    let layout = BitLayout::<U512>::new(vec![("x", 32)]).unwrap();
    let mut space = Space::<U512>::new("det", layout);
    space
        .indexable_attribute::<u32, _>("x", "x", |v| v as u32)
        .unwrap();
    let mut world = World::<U512>::new();
    world.register(space).unwrap();
    let s = world.space("det").unwrap();

    let cohort = s.find().where_eq("x", 42).into_cohort();
    let a = cohort.sample(5);
    let b = cohort.sample(5);
    assert_eq!(a, b, "sample not deterministic");
}
