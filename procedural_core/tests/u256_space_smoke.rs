//! End-to-end smoke test for `Space<U256>`: build a layout, register
//! attributes, register the space in a `World`, run a `find()` query
//! with pushdown predicates. Catches any path that assumes ≤ u128.

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::word::U256;
use procedural_core::world::World;

fn build_wide_space() -> Space<U256> {
    // 200-bit layout (exceeds u128) — uses all four limbs.
    let layout = BitLayout::<U256>::new(vec![
        ("low", 64),
        ("mid_lo", 64),
        ("mid_hi", 32),
        ("hi", 40),
    ])
    .expect("200-bit layout fits in U256");
    let mut space = Space::<U256>::new("wide", layout);
    for f in ["low", "mid_lo", "mid_hi", "hi"] {
        space
            .indexable_attribute::<u64, _>(f, f, |v| v)
            .expect("register attribute");
    }
    space
}

#[test]
fn u256_layout_total_width() {
    let layout = BitLayout::<U256>::new(vec![("a", 100), ("b", 100)]).expect("200-bit layout");
    assert_eq!(layout.total_width(), 200);
}

#[test]
fn u256_layout_round_trip_compose_extract() {
    let layout = BitLayout::<U256>::new(vec![
        ("low", 64),
        ("mid_lo", 64),
        ("mid_hi", 32),
        ("hi", 40),
    ])
    .expect("200-bit layout");
    let id = layout.compose(&[
        ("low", 0xDEAD_BEEF_CAFE_F00D),
        ("mid_lo", 0xFEED_FACE_BAD0_C0DE),
        ("mid_hi", 0x1234_5678),
        ("hi", 0xAB_CDEF_0123),
    ]);
    assert_eq!(layout.extract(id, "low"), 0xDEAD_BEEF_CAFE_F00D);
    assert_eq!(layout.extract(id, "mid_lo"), 0xFEED_FACE_BAD0_C0DE);
    assert_eq!(layout.extract(id, "mid_hi"), 0x1234_5678);
    assert_eq!(layout.extract(id, "hi"), 0xAB_CDEF_0123);
}

#[test]
fn u256_world_registers_space() {
    let mut world = World::<U256>::new();
    world
        .register(build_wide_space())
        .expect("register wide space");
    assert!(world.has_space("wide"));
}

#[test]
fn u256_find_where_eq_pushdown() {
    let mut world = World::<U256>::new();
    world
        .register(build_wide_space())
        .expect("register wide space");
    let space = world.space("wide").expect("get wide space");
    let layout = BitLayout::<U256>::new(vec![
        ("low", 64),
        ("mid_lo", 64),
        ("mid_hi", 32),
        ("hi", 40),
    ])
    .expect("200-bit layout");
    let target = layout.compose(&[("low", 7), ("mid_lo", 11), ("mid_hi", 13), ("hi", 17)]);
    let results: Vec<_> = space
        .find()
        .where_eq("low", 7)
        .where_eq("mid_lo", 11)
        .where_eq("mid_hi", 13)
        .where_eq("hi", 17)
        .execute()
        .collect();
    assert_eq!(results, vec![target]);
}
