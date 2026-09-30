//! End-to-end two-space integration: people + families linked via cross-space attributes,
//! plus a global time function that both spaces' attributes consume.

use chrono::{TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::hash::hash_float;
use procedural_core::space::Space;
use procedural_core::world::World;

#[test]
fn two_spaces_with_cross_space_and_global() {
    // families: indexable name + static score attribute
    let families_layout = BitLayout::<u64>::new(vec![("fam_id", 16), ("rest", 48)]).unwrap();
    let mut families = Space::<u64>::new("families", families_layout);
    families
        .indexable_attribute::<String, _>("surname", "fam_id", |b| format!("Fam{}", b))
        .unwrap();
    families
        .attribute::<f64, _>("wealth", |id| hash_float(id, "wealth"))
        .unwrap();

    // people: indexable family_id + cross-space attribute that reads family surname
    let people_layout = BitLayout::<u64>::new(vec![("family_id", 16), ("entropy", 48)]).unwrap();
    let mut people = Space::<u64>::new("people", people_layout.clone());
    people
        .indexable_attribute::<u64, _>("family_id", "family_id", |b| b)
        .unwrap();

    let layout_for_surname = people_layout.clone();
    people
        .cross_space_attribute::<String, _>("surname", move |id: u64, world: &World<u64>| {
            let fam = layout_for_surname.extract(id, "family_id");
            world
                .attribute_value::<String>("families", fam, "surname", None)
                .unwrap_or_else(|_| "unknown".to_string())
        })
        .unwrap();

    // cross-space temporal: net wealth adjusted by global economy
    let layout_for_wealth = people_layout;
    people
        .cross_space_temporal_attribute::<f64, _>(
            "adjusted_wealth",
            move |id: u64, t, world: &World<u64>| {
                let fam = layout_for_wealth.extract(id, "family_id");
                let family_wealth: f64 = world
                    .attribute_value("families", fam, "wealth", None)
                    .unwrap_or(0.0);
                let economy: f64 = world.global("economy", t).unwrap_or(1.0);
                family_wealth * economy
            },
        )
        .unwrap();

    let mut world: World<u64> = World::new();
    world
        .register_global::<f64, _>("economy", |t| {
            // Deterministic "economy" value at time t
            hash_float(t.timestamp() as u64, "economy") * 2.0
        })
        .unwrap();
    world.register(families).unwrap();
    world.register(people).unwrap();

    // Build alice with family_id = 7
    let alice = 7u64; // low 16 bits are family_id=7, rest zero
    let surname: String = world
        .attribute_value("people", alice, "surname", None)
        .unwrap();
    assert_eq!(surname, "Fam7");

    let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
    let adjusted: f64 = world
        .attribute_value("people", alice, "adjusted_wealth", Some(t))
        .unwrap();
    let fam_wealth: f64 = world
        .attribute_value("families", 7u64, "wealth", None)
        .unwrap();
    let economy: f64 = world.global("economy", t).unwrap();
    assert!((adjusted - fam_wealth * economy).abs() < 1e-9);

    // Snapshot query covers all attributes
    let snap = world.entity("people", alice, Some(t)).unwrap();
    assert_eq!(*snap.get::<String>("surname").unwrap(), "Fam7");

    // Introspection
    let mut space_names: Vec<_> = world.space_names().collect();
    space_names.sort();
    assert_eq!(
        space_names,
        vec!["families".to_string(), "people".to_string()]
    );
    let global_names: Vec<_> = world.global_names().collect();
    assert_eq!(global_names, vec!["economy".to_string()]);
}

#[test]
fn composite_attribute_routes_through_world() {
    use procedural_core::bits::BitLayout;
    use procedural_core::space::Space;
    use procedural_core::world::World;

    let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4), ("entropy", 8)]).unwrap();
    let mut space = Space::<u64>::new("s", layout.clone());
    space
        .indexable_composite_attribute::<String, _>("label", "own", &["shared"], |own, dep| {
            format!("{}:{}", own, dep[0])
        })
        .unwrap();

    let mut world = World::<u64>::new();
    world.register(space).unwrap();

    let id = layout.compose(&[("own", 3), ("shared", 5)]);
    let v: String = world.attribute_value("s", id, "label", None).unwrap();
    assert_eq!(v, "3:5");

    // `entity` snapshot path also covers composite.
    let snap = world.entity("s", id, None).unwrap();
    assert_eq!(*snap.get::<String>("label").unwrap(), "3:5");
}
