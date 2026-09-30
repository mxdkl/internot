//! End-to-end integration: build a small "people" space with all three attribute
//! kinds (indexable, computed static, temporal), declare a relation and a
//! context, and query via entity snapshot.

use chrono::{Datelike, TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::hash::{hash_float, hash_int};
use procedural_core::space::{
    context::{ContextDim, Metric, Normalization},
    Arity, Space,
};

#[test]
fn people_space_end_to_end() {
    let layout = BitLayout::<u64>::new(vec![
        ("gen", 4),
        ("family", 24),
        ("home_locale", 8),
        ("birth_year_offset", 7),
        ("entropy", 21),
    ])
    .unwrap();

    let mut people = Space::<u64>::new("people", layout.clone());

    people
        .indexable_attribute::<String, _>("home_locale", "home_locale", |bits| {
            format!("locale_{}", bits)
        })
        .unwrap();

    people
        .indexable_attribute::<i32, _>("birth_year", "birth_year_offset", |bits| 1950 + bits as i32)
        .unwrap();

    people
        .attribute::<f64, _>("openness", |id| hash_float(id, "openness"))
        .unwrap();

    people
        .temporal_attribute::<f64, _>("age", |id, t| {
            let birth_year = 1950 + hash_int(id, "birth_year_offset", 80) as i32;
            (t.year() - birth_year) as f64
        })
        .unwrap();

    people
        .relation(
            "parents",
            Arity::Fixed(2),
            |_id, _t, _idx| 0u64, // declaration-only here; enumeration tested in relations_integration.rs
        )
        .unwrap();

    people
        .context(
            "friendship",
            vec![ContextDim {
                attribute: "openness".into(),
                weight: 1.0,
                normalize: Normalization::None,
            }],
            Metric::Cosine,
        )
        .unwrap();

    let alice = layout.compose(&[("home_locale", 5), ("birth_year_offset", 35)]);
    let t = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();

    // Single-attribute queries
    let locale: String = people.attribute_value(alice, "home_locale", None).unwrap();
    assert_eq!(locale, "locale_5");
    let birth: i32 = people.attribute_value(alice, "birth_year", None).unwrap();
    assert_eq!(birth, 1985);
    let openness: f64 = people.attribute_value(alice, "openness", None).unwrap();
    assert!((0.0..1.0).contains(&openness));
    let age: f64 = people.attribute_value(alice, "age", Some(t)).unwrap();
    assert!(age >= 0.0);

    // Snapshot query
    let snap = people.entity(alice, Some(t));
    assert_eq!(*snap.get::<String>("home_locale").unwrap(), "locale_5");
    assert_eq!(*snap.get::<i32>("birth_year").unwrap(), 1985);
    assert!(snap.errors().is_empty());

    // Introspection
    let attrs = people.attribute_names();
    assert_eq!(attrs.len(), 4);
    assert!(people.has_relation("parents"));
    assert!(people.has_context("friendship"));
}
