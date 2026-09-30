//! Canonical snapshot tests.
//!
//! Locks in every Layer 0 primitive's output at a fixed input set. Any
//! unintended drift in hashing, bit-layout, trajectory, sampler, or edge
//! function will fail the snapshot visibly. Intentional changes must be
//! accepted via `cargo insta review`.

use chrono::{Duration, TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::edge::{block, cosine, geometric, hyperbolic};
use procedural_core::hash::{hash_float, hash_gaussian, hash_int, hash_vec};
use procedural_core::sampler::{categorical, exponential, lognormal, pareto};
use procedural_core::trajectory::{
    oscillate, oscillate_stability_radius, smooth, smooth_stability_radius,
    stability_radius_quadratic, step, step_stability_radius,
};

#[test]
fn canonical_primitives_snapshot() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();

    let snapshot = format!(
        "hash_float(42u64, \"age\") = {}\n\
         hash_int(42u64, \"locale\", 256) = {}\n\
         hash_vec(42u64, \"personality\", 5) = {:?}\n\
         hash_gaussian(42u64, \"z\") = {}\n\
         \n\
         smooth(42u64, t0, 1.0, 30d) = {}\n\
         step(42u64, t0, [2020-01-01], [5.0]) = {}\n\
         oscillate(42u64, t0, 7d, 0.5, 0.0) = {}\n\
         \n\
         stability_radius_quadratic(10.0, 0.0, 1.0) = {}\n\
         stability_radius_quadratic(0.0, 8.0, 1.0) = {}\n\
         stability_radius_quadratic(5.0, 2.0, 0.1) = {}\n\
         smooth_stability_radius(42u64, t0, 1.0, 30d, 0.05) = {}s\n\
         oscillate_stability_radius(42u64, t0, 7d, 1.0, 0.3, 0.05) = {}s\n\
         step_stability_radius(42u64, t0, [2020-01-01, 2030-01-01], [5.0, 10.0], 1.0) = {}s\n\
         \n\
         pareto(42u64, \"size\", 1.5, 1.0) = {}\n\
         lognormal(42u64, \"income\", 10.0, 0.5) = {}\n\
         exponential(42u64, \"wait\", 2.0) = {}\n\
         categorical(42u64, \"class\", [0.1, 0.6, 0.3]) = {}\n\
         \n\
         geometric([0,0,0], [1,0,0], 2.0, true) = {}\n\
         cosine([1,0,0], [1,1,0], 0.0) = {}\n\
         hyperbolic((0.5, 0.0), (0.5, 1.0), 5.0, 0.3) = {}\n\
         block(5, 7, 0.8, 0.1) = {}\n\
         \n\
         BitLayout.compose(gen=3, fam=0x1234567, sib=7, ent=0x89ABCDE) = 0x{:x}\n",
        hash_float(42u64, "age"),
        hash_int(42u64, "locale", 256),
        hash_vec(42u64, "personality", 5),
        hash_gaussian(42u64, "z"),
        smooth(42u64, t0, 1.0, Duration::days(30)),
        step(
            42u64,
            t0,
            &[Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap()],
            &[5.0],
        ),
        oscillate(42u64, t0, Duration::days(7), 0.5, 0.0),
        stability_radius_quadratic(10.0, 0.0, 1.0),
        stability_radius_quadratic(0.0, 8.0, 1.0),
        stability_radius_quadratic(5.0, 2.0, 0.1),
        smooth_stability_radius(42u64, t0, 1.0, Duration::days(30), 0.05)
            .num_nanoseconds()
            .unwrap_or(i64::MAX),
        oscillate_stability_radius(42u64, t0, Duration::days(7), 1.0, 0.3, 0.05)
            .num_nanoseconds()
            .unwrap_or(i64::MAX),
        step_stability_radius(
            42u64,
            t0,
            &[
                Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            ],
            &[5.0, 10.0],
            1.0,
        )
        .num_seconds(),
        pareto(42u64, "size", 1.5, 1.0),
        lognormal(42u64, "income", 10.0, 0.5),
        exponential(42u64, "wait", 2.0),
        categorical(42u64, "class", &[0.1, 0.6, 0.3]),
        geometric(&[0.0, 0.0, 0.0], &[1.0, 0.0, 0.0], 2.0, true),
        cosine(&[1.0, 0.0, 0.0], &[1.0, 1.0, 0.0], 0.0),
        hyperbolic((0.5, 0.0), (0.5, 1.0), 5.0, 0.3),
        block(5, 7, 0.8, 0.1),
        {
            let layout =
                BitLayout::<u64>::new(vec![("gen", 4), ("fam", 28), ("sib", 4), ("ent", 28)])
                    .unwrap();
            layout.compose(&[
                ("gen", 3),
                ("fam", 0x1234567),
                ("sib", 7),
                ("ent", 0x89ABCDE),
            ])
        },
    );

    insta::assert_snapshot!("primitives", snapshot);
}

/// Second canonical snapshot: verifies u128 outputs are stable.
/// This locks in that u128 hashes produce different-but-deterministic outputs.
#[test]
fn canonical_u128_snapshot() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
    let id: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;

    let snapshot = format!(
        "hash_float({:#x}, \"age\") = {}\n\
         hash_int({:#x}, \"locale\", 256) = {}\n\
         hash_gaussian({:#x}, \"z\") = {}\n\
         smooth({:#x}, t0, 1.0, 30d) = {}\n\
         pareto({:#x}, \"size\", 1.5, 1.0) = {}\n\
         lognormal({:#x}, \"income\", 10.0, 0.5) = {}\n\
         BitLayout<u128>.compose(a=0xAB, b=0xFFFF, c=0xCDE, d=0x0FFFFFFFFFFFFFFF) = 0x{:x}\n",
        id,
        hash_float(id, "age"),
        id,
        hash_int(id, "locale", 256),
        id,
        hash_gaussian(id, "z"),
        id,
        smooth(id, t0, 1.0, Duration::days(30)),
        id,
        pareto(id, "size", 1.5, 1.0),
        id,
        lognormal(id, "income", 10.0, 0.5),
        {
            let layout =
                BitLayout::<u128>::new(vec![("a", 8), ("b", 16), ("c", 12), ("d", 60)]).unwrap();
            layout.compose(&[
                ("a", 0xAB),
                ("b", 0xFFFF),
                ("c", 0xCDE),
                ("d", 0x0FFFFFFFFFFFFFFF),
            ])
        },
    );

    insta::assert_snapshot!("primitives_u128", snapshot);
}

#[test]
fn canonical_space_snapshot() {
    use procedural_core::bits::BitLayout;
    use procedural_core::hash::hash_float;
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("locale", 8), ("rest", 56)]).unwrap();
    let mut space = Space::<u64>::new("anchor", layout.clone());
    space
        .indexable_attribute::<String, _>("locale_name", "locale", |b| format!("L{}", b))
        .unwrap();
    space
        .attribute::<f64, _>("score", |id| hash_float(id, "score"))
        .unwrap();

    let id = layout.compose(&[("locale", 0xAB)]);
    let locale: String = space.attribute_value(id, "locale_name", None).unwrap();
    let score: f64 = space.attribute_value(id, "score", None).unwrap();

    let snapshot = format!(
        "Space<u64>.attribute_value(locale_name, 0x{:x}) = {}\n\
         Space<u64>.attribute_value(score,       0x{:x}) = {}\n",
        id, locale, id, score,
    );

    insta::assert_snapshot!("space_layer", snapshot);
}

#[test]
fn canonical_world_snapshot() {
    use procedural_core::bits::BitLayout;
    use procedural_core::hash::hash_float;
    use procedural_core::space::Space;
    use procedural_core::world::World;

    let layout_a = BitLayout::<u64>::new(vec![("ref", 16), ("rest", 48)]).unwrap();
    let layout_b = BitLayout::<u64>::new(vec![("id_b", 16), ("rest", 48)]).unwrap();
    let mut a = Space::<u64>::new("a", layout_a.clone());
    let mut b = Space::<u64>::new("b", layout_b);
    b.attribute::<f64, _>("value", |id| hash_float(id, "value"))
        .unwrap();

    let layout_for_closure = layout_a;
    a.cross_space_attribute::<f64, _>("linked_value", move |id: u64, world: &World<u64>| {
        let r = layout_for_closure.extract(id, "ref");
        world
            .attribute_value::<f64>("b", r, "value", None)
            .unwrap_or(-1.0)
    })
    .unwrap();

    let mut world: World<u64> = World::new();
    world.register(a).unwrap();
    world.register(b).unwrap();
    world
        .register_global::<f64, _>("zeitgeist", |t| {
            hash_float(t.timestamp() as u64, "zeitgeist")
        })
        .unwrap();

    // ref = 42 -> read b.value(42)
    let id = 42u64;
    let cross: f64 = world
        .attribute_value("a", id, "linked_value", None)
        .unwrap();

    use chrono::TimeZone;
    let t = chrono::Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
    let g: f64 = world.global("zeitgeist", t).unwrap();

    let snapshot = format!(
        "World.attribute_value(a, 42, linked_value) = {}\n\
         World.global(zeitgeist, 2026-04-21T12:00:00Z) = {}\n",
        cross, g,
    );

    insta::assert_snapshot!("world", snapshot);
}

#[test]
fn canonical_relations_snapshot() {
    use chrono::{TimeZone, Utc};
    use procedural_core::bits::BitLayout;
    use procedural_core::space::arity::Arity;
    use procedural_core::space::Space;
    use procedural_core::world::World;

    let families_layout = BitLayout::<u64>::new(vec![("family_id", 28), ("entropy", 36)]).unwrap();
    let mut families = Space::<u64>::new("families", families_layout);

    families
        .relation("parents", Arity::Fixed(2), |fam, _t, idx| {
            0x8000_0000_0000_0000 | (fam << 8) | (idx as u64)
        })
        .unwrap();

    families
        .relation(
            "children",
            Arity::dynamic(|fam: u64, _t| ((fam.wrapping_mul(2654435761)) % 5) as usize),
            |fam, _t, idx| (fam << 8) | (idx as u64),
        )
        .unwrap();

    let mut world: World<u64> = World::new();
    world.register(families).unwrap();

    let t = Utc.with_ymd_and_hms(2026, 4, 22, 12, 0, 0).unwrap();

    let fam: u64 = 7;
    let parents: Vec<u64> = world
        .related("families", fam, "parents", t)
        .unwrap()
        .collect();
    let children: Vec<u64> = world
        .related("families", fam, "children", t)
        .unwrap()
        .collect();

    let snapshot = format!(
        "World.related(families, {fam}, parents) = {:?}\n\
         World.related(families, {fam}, children) count = {}\n\
         World.related(families, {fam}, children) = {:?}\n",
        parents,
        children.len(),
        children,
    );

    insta::assert_snapshot!("relations", snapshot);
}

#[test]
fn canonical_search_patterns_snapshot() {
    use procedural_core::search::{minimize_patterns, range_to_prefixes};

    let range_1_6_3 = range_to_prefixes(1, 6, 3);
    let range_16_25_7 = range_to_prefixes(16, 25, 7);
    let set_odd_3 = minimize_patterns(&[1, 3, 5, 7], 3);
    let set_dense_4 = minimize_patterns(&(0u64..8).collect::<Vec<_>>(), 4);

    let render = |patterns: &[procedural_core::search::BitPattern]| -> String {
        patterns
            .iter()
            .map(|p| format!("{{fixed: 0b{:08b}, mask: 0b{:08b}}}", p.fixed, p.mask))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let snapshot = format!(
        "range_to_prefixes(1, 6, 3) = [{}]\n\
         range_to_prefixes(16, 25, 7) = [{}]\n\
         minimize_patterns([1,3,5,7], 3) count = {}\n\
         minimize_patterns([1,3,5,7], 3) = [{}]\n\
         minimize_patterns([0..8], 4) = [{}]\n",
        render(&range_1_6_3),
        render(&range_16_25_7),
        set_odd_3.len(),
        render(&set_odd_3),
        render(&set_dense_4),
    );

    insta::assert_snapshot!("search_patterns", snapshot);
}

#[test]
fn canonical_search_enumerate_snapshot() {
    use procedural_core::search::{range_to_prefixes, BitPattern};

    let p1 = BitPattern::new(0b1000, 0b1100);
    let ids_p1: Vec<u64> = p1.enumerate(4).collect();

    let p2 = BitPattern::any(3);
    let ids_p2: Vec<u64> = p2.enumerate(3).collect();

    let range_3_12 = range_to_prefixes(3, 12, 4);
    let mut range_ids: Vec<u64> = range_3_12.iter().flat_map(|p| p.enumerate(4)).collect();
    range_ids.sort_unstable();

    let snapshot = format!(
        "BitPattern{{0b1000, 0b1100}}.enumerate(4) = {:?}\n\
         BitPattern::any(3).enumerate(3) = {:?}\n\
         range_to_prefixes(3, 12, 4) enumerated+sorted = {:?}\n",
        ids_p1, ids_p2, range_ids,
    );

    insta::assert_snapshot!("search_enumerate", snapshot);
}

#[test]
fn canonical_find_snapshot() {
    use procedural_core::bits::BitLayout;
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);

    let pinned_a: Vec<u64> = space.find().where_eq("a", 3).execute().collect();
    let range_b: Vec<u64> = space
        .find()
        .where_eq("a", 3)
        .where_range("b", 5, 10)
        .execute()
        .collect();
    let set_ab: Vec<u64> = space
        .find()
        .where_in("a", &[1, 2])
        .where_in("b", &[0, 4, 8])
        .execute()
        .collect();

    let mut sorted_pinned_a = pinned_a.clone();
    sorted_pinned_a.sort_unstable();
    let mut sorted_range_b = range_b.clone();
    sorted_range_b.sort_unstable();
    let mut sorted_set_ab = set_ab.clone();
    sorted_set_ab.sort_unstable();

    let snapshot = format!(
        "where_eq(a, 3) count = {}, sorted = {:?}\n\
         where_eq(a, 3) + where_range(b, 5, 10) count = {}, sorted = {:?}\n\
         where_in(a, [1,2]) + where_in(b, [0,4,8]) count = {}, sorted = {:?}\n",
        pinned_a.len(),
        sorted_pinned_a,
        range_b.len(),
        sorted_range_b,
        set_ab.len(),
        sorted_set_ab,
    );

    insta::assert_snapshot!("find", snapshot);
}

#[test]
fn canonical_find_filters_snapshot() {
    use procedural_core::bits::BitLayout;
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);

    // Pin a=3, filter b even, with budget=20 (greater than total candidates 16).
    let mut results = space
        .find()
        .where_eq("a", 3)
        .filter_static("b_even", |id| (id >> 4) & 1 == 0)
        .scan_budget(20)
        .execute();
    let mut yielded: Vec<u64> = (&mut results).collect();
    yielded.sort_unstable();
    let evaluated = results.evaluated();
    let term = results.termination().clone();

    let snapshot = format!(
        "where_eq(a, 3) + filter(b_even) + scan_budget(20):\n\
         yielded count = {}, sorted = {:?}\n\
         evaluated = {}\n\
         termination = {:?}\n",
        yielded.len(),
        yielded,
        evaluated,
        term,
    );

    insta::assert_snapshot!("find_filters", snapshot);
}

#[test]
fn canonical_find_temporal_snapshot() {
    use chrono::{TimeZone, Utc};
    use procedural_core::bits::BitLayout;
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);
    let anchor = Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();

    let mut results = space
        .find()
        .where_eq("a", 3)
        .at(anchor)
        .filter_temporal("b_even_at_anchor", move |id, t| {
            assert_eq!(t, anchor);
            (id >> 4) & 1 == 0
        })
        .scan_budget(20)
        .execute();
    let mut ids: Vec<u64> = (&mut results).collect();
    ids.sort();
    let summary = format!(
        "where_eq(a, 3) + at(anchor) + filter_temporal(b_even) + scan_budget(20): \
         yielded count = {}, sorted = {:?}, evaluated = {}, termination = {:?}",
        ids.len(),
        ids,
        results.evaluated(),
        results.termination()
    );
    insta::assert_snapshot!("canonical_find_temporal", summary);
}

#[test]
fn canonical_find_temporal_range_snapshot() {
    use chrono::{Duration, TimeZone, Utc};
    use procedural_core::bits::BitLayout;
    use procedural_core::search::{Op, StabilityMode};
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);
    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
    let layout2 = space.layout().clone();

    let mut results = space
        .find()
        .where_eq("a", 3)
        .at_any(start, end)
        .filter_exists_in_range(
            "b_gt_7",
            move |id, _t| layout2.extract(id, "b") as f64,
            Op::Gt,
            7.0,
            StabilityMode::Analytic(Box::new(|_id, _t, _eps| Duration::hours(12))),
        )
        .scan_budget(20)
        .execute();
    let mut ids: Vec<u64> = (&mut results).collect();
    ids.sort();
    let summary = format!(
        "where_eq(a, 3) + at_any(1d) + filter_exists_in_range(b>7) + scan_budget(20): \
         yielded count = {}, sorted = {:?}, evaluated = {}, termination = {:?}",
        ids.len(),
        ids,
        results.evaluated(),
        results.termination()
    );
    insta::assert_snapshot!("canonical_find_temporal_range", summary);
}

#[test]
fn canonical_candidates_snapshot() {
    use procedural_core::bits::BitLayout;
    use procedural_core::space::context::{ContextDim, Metric, Normalization};
    use procedural_core::space::Space;

    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let mut space = Space::<u64>::new("tiny", layout);
    space.indexable_attribute("a", "a", |bits| bits).unwrap();
    space.indexable_attribute("b", "b", |bits| bits).unwrap();
    space
        .context(
            "simple",
            vec![
                ContextDim {
                    attribute: "a".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
                ContextDim {
                    attribute: "b".to_string(),
                    weight: 1.0,
                    normalize: Normalization::None,
                },
            ],
            Metric::Euclidean,
        )
        .unwrap();

    let results = space.candidates(0x35u64, "simple").budget(20).take(5);

    let hits_fmt: Vec<String> = results
        .hits
        .iter()
        .map(|(id, s)| format!("({id}, {:.4})", s))
        .collect();

    let summary = format!(
        "candidates(0x35, simple, euclidean) + budget(20) + take(5):\n\
         hits = [{}]\n\
         evaluated = {}, total_considered = {}, budget = {}",
        hits_fmt.join(", "),
        results.evaluated,
        results.total_considered,
        results.budget,
    );
    insta::assert_snapshot!("canonical_candidates", summary);
}
