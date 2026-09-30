//! End-to-end test for Query<Bounded>::at() + filter_temporal().
//!
//! Uses a realistic two-field layout and a trajectory primitive
//! (oscillate) inside the temporal closure to validate that
//! user-defined temporal attributes work as designed.

use chrono::{Duration, TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::trajectory::oscillate;

fn people_space() -> Space<u64> {
    // baseline: 8 bits (0..256), volatility: 4 bits (0..16).
    let layout = BitLayout::<u64>::new(vec![("baseline", 8), ("volatility", 4)]).unwrap();
    Space::<u64>::new("people", layout)
}

#[test]
fn temporal_filter_at_a_fixed_time_narrows_to_expected_ids() {
    let space = people_space();
    let t = Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();

    // Narrow by baseline to keep enumeration bounded, then temporally
    // filter by an oscillating "current value" crossing a threshold.
    let layout = space.layout().clone();
    let layout2 = layout.clone();
    let ids: Vec<u64> = space
        .find()
        .where_range("baseline", 120, 140)
        .at(t)
        .filter_temporal("current_gt_130", move |id, t| {
            let baseline = layout.extract(id, "baseline") as f64;
            let volatility = layout.extract(id, "volatility") as f64;
            let current = baseline + oscillate(id, t, Duration::days(1), 2.0 * volatility, 0.0);
            current > 130.0
        })
        .execute()
        .collect();

    // Structural check: every yielded id has baseline in [120, 140]
    // and the closure evaluates true at t.
    for id in &ids {
        let baseline = layout2.extract(*id, "baseline");
        assert!((120..=140).contains(&baseline));
        let volatility = layout2.extract(*id, "volatility") as f64;
        let current = baseline as f64 + oscillate(*id, t, Duration::days(1), 2.0 * volatility, 0.0);
        assert!(current > 130.0, "id {id}: current={current}");
    }
    // With baseline ∈ [120, 140] (21 values) × volatility ∈ 0..16 (16 values)
    // = 336 candidates, we expect some (non-empty) subset to pass.
    assert!(!ids.is_empty(), "expected at least one passing id");
    assert!(ids.len() <= 336, "can't exceed candidate count");
}

#[test]
fn temporal_filter_terminates_exhaustive_when_drained() {
    let space = people_space();
    let t = Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();

    let mut results = space
        .find()
        .where_eq("baseline", 100)
        .at(t)
        .filter_temporal("always_true", |_id, _t| true)
        .execute();
    let ids: Vec<u64> = (&mut results).collect();

    // baseline=100 pinned, volatility ∈ 0..16 → 16 candidates, all pass.
    assert_eq!(ids.len(), 16);
    assert_eq!(
        results.termination(),
        &procedural_core::search::Termination::Exhaustive
    );
    assert_eq!(results.evaluated(), 16);
}

#[test]
fn temporal_filter_budget_caps_evaluations() {
    let space = people_space();
    let t = Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();

    let mut results = space
        .find()
        .where_range("baseline", 0, 255)
        .at(t)
        .scan_budget(50)
        .filter_temporal("always_true", |_id, _t| true)
        .execute();
    let _: Vec<u64> = (&mut results).collect();

    assert_eq!(results.evaluated(), 50);
    assert_eq!(
        results.termination(),
        &procedural_core::search::Termination::Budgeted { evaluated: 50 }
    );
}
