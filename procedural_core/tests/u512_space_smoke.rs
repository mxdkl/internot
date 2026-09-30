//! End-to-end smoke test for `Space<U512>`. Validates that a BitLayout
//! can be registered in a `World<U512>`, queried via `find().where_eq()`
//! with pattern pushdown, and yields the expected candidates.
//!
//! Mirrors `tests/u256_space_smoke.rs` for the wider word type that
//! the Person space depends on. Uses a 32-bit layout (the People
//! layout's planned width); a wider-layout test is added in
//! `find_iter` once the pre-flight 64-bit-free-bits panic is removed.

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::word::{BitWord, U512};
use procedural_core::world::World;

#[test]
fn u512_space_round_trips_through_find_pushdown() {
    // 32-bit layout matching the planned People layout
    // (member_idx:12, workplace_seed:8, city_idx:6, industry_idx:6).
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
        .expect("register industry_idx");
    space
        .indexable_attribute::<u32, _>("city_idx", "city_idx", |v| v as u32)
        .expect("register city_idx");

    let mut world = World::<U512>::new();
    world.register(space).expect("register smoke space");

    let smoke_space = world.space("smoke").expect("smoke space registered");
    let results: Vec<U512> = smoke_space
        .find()
        .where_eq("industry_idx", 5)
        .where_eq("city_idx", 3)
        .scan_budget(1024)
        .execute()
        .into_iter()
        .take(8)
        .collect();

    // Each result should have industry_idx=5 (bits 26..32) and city_idx=3 (bits 20..26).
    assert!(!results.is_empty(), "expected at least one match");
    for id in &results {
        assert_eq!(id.extract_bits(26, 6), 5, "industry_idx mismatch");
        assert_eq!(id.extract_bits(20, 6), 3, "city_idx mismatch");
    }
}
