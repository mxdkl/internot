//! End-to-end test: two spaces (families + people), cross-space relation
//! enumeration via World, consumer-side sibling composition.

use chrono::{TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::space::arity::Arity;
use procedural_core::space::Space;
use procedural_core::world::World;

/// Compose a person ID given a family ID and a child index.
/// Simple deterministic scheme: low 8 bits are child_index,
/// bits 8..36 hold family_id, rest zero.
fn compose_child_person_id(family_id: u64, child_index: usize) -> u64 {
    (family_id << 8) | (child_index as u64 & 0xFF)
}

/// Compose a parent person ID given a family ID and parent index (0=mother, 1=father).
fn compose_parent_person_id(family_id: u64, parent_index: usize) -> u64 {
    // Parents are in a disjoint tag-space (bit 63 set) to avoid colliding with children.
    0x8000_0000_0000_0000 | (family_id << 8) | (parent_index as u64 & 0xFF)
}

/// Children count: deterministic function of family_id alone (time-invariant in this test).
fn children_count(family_id: u64, _t: chrono::DateTime<chrono::Utc>) -> usize {
    ((family_id.wrapping_mul(2654435761)) % 5) as usize
}

#[test]
fn relations_end_to_end_families_and_people() {
    let people_layout =
        BitLayout::<u64>::new(vec![("child_index", 8), ("family_id", 28), ("entropy", 28)])
            .unwrap();
    let families_layout = BitLayout::<u64>::new(vec![("family_id", 28), ("entropy", 36)]).unwrap();

    let people = Space::<u64>::new("people", people_layout.clone());
    let mut families = Space::<u64>::new("families", families_layout);

    families
        .relation("parents", Arity::Fixed(2), |fam, _t, idx| {
            compose_parent_person_id(fam, idx)
        })
        .unwrap();

    families
        .relation(
            "children",
            Arity::dynamic(children_count),
            |fam, _t, idx| compose_child_person_id(fam, idx),
        )
        .unwrap();

    let mut world: World<u64> = World::new();
    world.register(people).unwrap();
    world.register(families).unwrap();

    let t = Utc.with_ymd_and_hms(2026, 4, 22, 12, 0, 0).unwrap();

    // Pick a family with >= 2 children deterministically.
    let family_id: u64 = 7;
    assert!(children_count(family_id, t) >= 2, "pick a richer family_id");

    // Parents (fixed arity = 2):
    let parents: Vec<u64> = world
        .related("families", family_id, "parents", t)
        .unwrap()
        .collect();
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0], compose_parent_person_id(family_id, 0));
    assert_eq!(parents[1], compose_parent_person_id(family_id, 1));

    // Children (dynamic arity):
    let expected_n = children_count(family_id, t);
    let children: Vec<u64> = world
        .related("families", family_id, "children", t)
        .unwrap()
        .collect();
    assert_eq!(children.len(), expected_n);
    for (i, &cid) in children.iter().enumerate() {
        assert_eq!(cid, compose_child_person_id(family_id, i));
    }

    // Consumer-side sibling composition: given alice (a person),
    // extract her family_id, query children, filter self.
    let alice_child_index = 1;
    let alice_id = compose_child_person_id(family_id, alice_child_index);
    let alice_family_id = people_layout.extract(alice_id, "family_id");
    assert_eq!(alice_family_id, family_id);

    let siblings: Vec<u64> = world
        .related("families", alice_family_id, "children", t)
        .unwrap()
        .filter(|&id| id != alice_id)
        .collect();
    assert_eq!(siblings.len(), expected_n - 1);
    assert!(!siblings.contains(&alice_id));
}
