//! End-to-end tests for Space::find() on a realistic people-like layout.

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;

fn people_space() -> Space<u64> {
    let layout = BitLayout::<u64>::new(vec![
        ("locale", 8), // 256 locales
        ("age", 7),    // 0..128
        ("hash", 49),  // entropy / unconstrained
    ])
    .unwrap();
    Space::<u64>::new("people", layout)
}

#[test]
fn where_eq_pinned_locale_enumerates_all_ages_and_hashes() {
    let space = people_space();
    // Pin locale=42, age=30 → 2^49 results.
    // That's too wide to materialize; just take the first few.
    let first_3: Vec<u64> = space
        .find()
        .where_eq("locale", 42)
        .where_eq("age", 30)
        .execute()
        .take(3)
        .collect();
    assert_eq!(first_3.len(), 3);
    for id in &first_3 {
        assert_eq!(space.layout().extract(*id, "locale"), 42);
        assert_eq!(space.layout().extract(*id, "age"), 30);
    }
}

#[test]
fn where_range_and_where_in_combine() {
    let space = people_space();
    // Pin locale ∈ {5,6,9}, age ∈ [20,25]. hash is unconstrained (49 bits).
    let first_20: Vec<u64> = space
        .find()
        .where_in("locale", &[5, 6, 9])
        .where_range("age", 20, 25)
        .execute()
        .take(20)
        .collect();
    assert_eq!(first_20.len(), 20);
    for id in &first_20 {
        let l = space.layout().extract(*id, "locale");
        let a = space.layout().extract(*id, "age");
        assert!([5u64, 6, 9].contains(&l));
        assert!((20..=25).contains(&a));
    }
}

#[test]
fn exhaust_narrow_query_exactly() {
    // Tiny layout, both fields pinned → 1 result.
    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);
    let ids: Vec<u64> = space
        .find()
        .where_eq("a", 3)
        .where_eq("b", 7)
        .execute()
        .collect();
    assert_eq!(ids.len(), 1);
    assert_eq!(space.layout().extract(ids[0], "a"), 3);
    assert_eq!(space.layout().extract(ids[0], "b"), 7);
}

#[test]
fn pin_one_field_in_small_layout_enumerates_other() {
    // 4-bit + 4-bit → 256 ids total. Pin a, expect 16 results.
    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);
    let ids: Vec<u64> = space.find().where_eq("a", 5).execute().collect();
    assert_eq!(ids.len(), 16);
}
