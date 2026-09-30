//! Shared fixture for `search` and `scale` benches.
//!
//! Construction cost is paid once per bench binary (not per iteration), so
//! this file builds a small world with the people+families layout used by
//! the `people_family` example, but without string tables — attribute
//! closures return raw u64 values. This keeps measurements focused on
//! framework overhead rather than string formatting.

use chrono::{DateTime, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::space::context::{ContextDim, Metric, Normalization};
use procedural_core::space::Space;
use procedural_core::world::World;

#[allow(dead_code)] // used by sibling bench files via `#[path]`
pub fn bench_world() -> World<u64> {
    let people_layout = BitLayout::<u64>::new(vec![
        ("entropy", 20),
        ("chronotype", 2),
        ("personality", 4),
        ("locale_idx", 4),
        ("occupation_idx", 5),
        ("age", 7),
        ("given_name_idx", 6),
        ("family_id", 16),
    ])
    .unwrap();
    let mut people = Space::<u64>::new("people", people_layout);

    for field in [
        "family_id",
        "given_name_idx",
        "age",
        "occupation_idx",
        "locale_idx",
        "personality",
        "chronotype",
    ] {
        people
            .indexable_attribute::<u64, _>(field, field, |b| b)
            .unwrap();
    }

    people
        .context(
            "vibe",
            vec![
                ContextDim {
                    attribute: "personality".into(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "chronotype".into(),
                    weight: 0.5,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "age".into(),
                    weight: 0.01,
                    normalize: Normalization::None,
                },
            ],
            Metric::Cosine,
        )
        .unwrap();
    people
        .context(
            "lifestyle",
            vec![
                ContextDim {
                    attribute: "locale_idx".into(),
                    weight: 2.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "occupation_idx".into(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "age".into(),
                    weight: 0.05,
                    normalize: Normalization::None,
                },
            ],
            Metric::Euclidean,
        )
        .unwrap();

    let families_layout = BitLayout::<u64>::new(vec![
        ("fam_id", 16),
        ("surname_idx", 6),
        ("home_locale", 4),
        ("size", 4),
        ("era", 4),
        ("wealth_bucket", 8),
        ("entropy", 22),
    ])
    .unwrap();
    let mut families = Space::<u64>::new("families", families_layout);
    for field in [
        "fam_id",
        "surname_idx",
        "home_locale",
        "size",
        "era",
        "wealth_bucket",
    ] {
        families
            .indexable_attribute::<u64, _>(field, field, |b| b)
            .unwrap();
    }

    let mut world = World::<u64>::new();
    world
        .register_global::<f64, _>("economy", |t: DateTime<Utc>| {
            let day = t.timestamp() as f64 / 86_400.0;
            0.3 * (2.0 * std::f64::consts::PI * day / 365.0).sin()
        })
        .unwrap();
    world.register(people).unwrap();
    world.register(families).unwrap();
    world
}

#[allow(dead_code)]
pub fn bench_reference_id(world: &World<u64>) -> u64 {
    let people = world.space("people").unwrap();
    people.layout().compose(&[
        ("family_id", 42),
        ("given_name_idx", 12),
        ("age", 34),
        ("occupation_idx", 5),
        ("locale_idx", 1),
        ("personality", 3),
        ("chronotype", 0),
        ("entropy", 0),
    ])
}
