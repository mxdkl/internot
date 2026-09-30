//! End-to-end tests for Space::candidates().
//!
//! Realistic 3-field layout with two distinct contexts (one Euclidean,
//! one Cosine) — validates scoring, envelope narrowing, and deterministic
//! output over the full pipeline.

use procedural_core::bits::BitLayout;
use procedural_core::space::context::{ContextDim, Metric, Normalization};
use procedural_core::space::Space;

fn people_space() -> Space<u64> {
    let layout = BitLayout::<u64>::new(vec![("locale", 8), ("occupation", 8), ("age", 8)]).unwrap();
    let mut space = Space::<u64>::new("people", layout);
    space
        .indexable_attribute("locale", "locale", |bits| bits)
        .unwrap();
    space
        .indexable_attribute("occupation", "occupation", |bits| bits)
        .unwrap();
    space
        .indexable_attribute("age", "age", |bits| bits)
        .unwrap();
    space
        .context(
            "cultural",
            vec![
                ContextDim {
                    attribute: "locale".to_string(),
                    weight: 2.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "occupation".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
            ],
            Metric::Euclidean,
        )
        .unwrap();
    space
        .context(
            "vibe",
            vec![
                ContextDim {
                    attribute: "occupation".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "age".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
            ],
            Metric::Cosine,
        )
        .unwrap();
    space
}

#[test]
fn cultural_euclidean_top_3_has_valid_scores() {
    let space = people_space();
    let query_id = 0x123456u64;
    let results = space.candidates(query_id, "cultural").budget(500).take(3);

    assert_eq!(results.hits.len(), 3);
    // Euclidean → non-positive scores, sorted descending.
    for (id, score) in &results.hits {
        assert!(*id != query_id);
        assert!(*score <= 0.0, "euclidean score {score} must be <= 0");
    }
    for w in results.hits.windows(2) {
        assert!(w[0].1 >= w[1].1);
    }
}

#[test]
fn vibe_cosine_scores_are_in_valid_range() {
    let space = people_space();
    let query_id = 0x654321u64;
    let results = space.candidates(query_id, "vibe").budget(500).take(5);

    for (_, score) in &results.hits {
        assert!(
            (-1.0..=1.0).contains(score),
            "cosine score {score} outside [-1, 1]"
        );
    }
}

#[test]
fn envelope_narrowing_restricts_to_shared_locale() {
    let space = people_space();
    let query_id = 0x7B_05_03u64; // locale=3, occupation=5, age=0x7B
    let results = space
        .candidates(query_id, "cultural")
        .envelope("locale")
        .budget(500)
        .take(20);

    for (id, _) in &results.hits {
        assert_eq!(space.layout().extract(*id, "locale"), 3);
        assert_ne!(*id, query_id);
    }
}

#[test]
fn results_metadata_is_consistent() {
    let space = people_space();
    let results = space.candidates(0u64, "cultural").budget(100).take(5);
    assert_eq!(results.budget, 100);
    assert_eq!(results.total_considered, 100);
    assert!(results.evaluated <= results.total_considered);
    assert!(results.hits.len() <= 5);
}

#[test]
fn composite_attribute_as_context_dim_scores_correctly() {
    // A composite numeric attribute — decoded from own + shared — used
    // directly as a similarity dim. Validates that
    // `search::candidates::eval_attribute` reads dep-field bits when it
    // evaluates a Composite slot.
    use procedural_core::bits::BitLayout;
    use procedural_core::space::context::{ContextDim, Metric, Normalization};
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("own", 4), ("shared", 4), ("entropy", 8)]).unwrap();
    let mut space = Space::<u64>::new("p", layout.clone());

    // Composite value = own * 16 + shared — a simple bijection onto [0, 255)
    // so we can predict similarity results.
    space
        .indexable_composite_attribute::<u64, _>("encoded", "own", &["shared"], |own, shared| {
            own * 16 + shared[0]
        })
        .unwrap();
    space
        .context(
            "composite_sim",
            vec![ContextDim {
                attribute: "encoded".to_string(),
                weight: 1.0,
                normalize: Normalization::None,
            }],
            Metric::Euclidean,
        )
        .unwrap();

    // Query id: own=5, shared=5 → encoded = 85.
    let query_id = layout.compose(&[("own", 5u64), ("shared", 5u64)]);
    let results = space
        .candidates(query_id, "composite_sim")
        .budget(2048)
        .take(5);

    // Every returned hit must have actually been evaluated through the
    // composite decoder. Top-1 should be extremely close to 85.
    assert!(!results.hits.is_empty(), "expected at least one hit");
    for (hit_id, score) in &results.hits {
        assert_ne!(*hit_id, query_id, "query id should be skipped");
        let own: u64 = space.layout().extract(*hit_id, "own");
        let shared: u64 = space.layout().extract(*hit_id, "shared");
        let encoded = own * 16 + shared;
        // Euclidean scoring: score = -|diff|  (framework returns negated
        // Euclidean for higher-is-better ordering). So the best score should
        // correspond to the smallest |encoded - 85|.
        let expected = -(((encoded as i64) - 85).abs() as f64);
        assert!(
            (score - expected).abs() < 1e-9,
            "score for id with encoded={} should be {} but was {}",
            encoded,
            expected,
            score
        );
    }
}

#[test]
fn composite_attribute_with_u128_layout() {
    // Same machinery on a u128 space — covers BitWord=u128 in the
    // composite eval paths of Space / World / candidates.
    use procedural_core::bits::BitLayout;
    use procedural_core::space::Space;
    use procedural_core::world::World;

    let layout = BitLayout::<u128>::new(vec![("own", 6), ("shared", 8), ("entropy", 16)]).unwrap();
    let mut space = Space::<u128>::new("wide", layout.clone());
    space
        .indexable_composite_attribute::<String, _>("label", "own", &["shared"], |own, shared| {
            format!("w{}.{}", own, shared[0])
        })
        .unwrap();

    let id = layout.compose(&[("own", 12u64), ("shared", 200u64)]);
    assert_eq!(
        space.attribute_value::<String>(id, "label", None).unwrap(),
        "w12.200"
    );

    let mut world = World::<u128>::new();
    world.register(space).unwrap();
    let v: String = world.attribute_value("wide", id, "label", None).unwrap();
    assert_eq!(v, "w12.200");
    let snap = world.entity("wide", id, None).unwrap();
    assert_eq!(*snap.get::<String>("label").unwrap(), "w12.200");
}
