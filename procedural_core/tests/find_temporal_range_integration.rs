//! End-to-end test for Query<Bounded>::at_any + filter_*_in_range.
//!
//! Uses a realistic baseline+volatility layout and an oscillating value
//! function to validate that the sweep engine finds threshold crossings
//! over a multi-day range.

use chrono::{Duration, TimeZone, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::search::{stability, Op, StabilityMode};
use procedural_core::space::Space;
use procedural_core::trajectory::{oscillate, oscillate_stability_radius};

fn people_space() -> Space<u64> {
    // baseline: 8 bits (0..256), volatility: 4 bits (0..16).
    let layout = BitLayout::<u64>::new(vec![("baseline", 8), ("volatility", 4)]).unwrap();
    Space::<u64>::new("people", layout)
}

#[test]
fn exists_in_range_finds_threshold_crossings_on_oscillating_trajectory() {
    let space = people_space();
    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 5, 4, 0, 0, 0).unwrap(); // 3 days
    let layout = space.layout().clone();

    let ids: Vec<u64> = space
        .find()
        .where_range("baseline", 130, 140)
        .at_any(start, end)
        .filter_exists_in_range(
            "crosses_145",
            move |id, t| {
                let baseline = layout.extract(id, "baseline") as f64;
                let volatility = layout.extract(id, "volatility") as f64;
                baseline + oscillate(id, t, Duration::days(1), 5.0 * volatility, 0.0)
            },
            Op::Gt,
            145.0,
            StabilityMode::Analytic(Box::new(|id, t, eps| {
                // Only one component, so no split needed — could use
                // oscillate_stability_radius directly. Demonstrate min_of.
                stability::min_of(
                    &[&|id, t, e| {
                        oscillate_stability_radius(id, t, Duration::days(1), 5.0, 0.0, e)
                    }],
                    id,
                    t,
                    eps,
                )
            })),
        )
        .execute()
        .collect();

    // Structural check: each yielded id has baseline ∈ [130, 140] AND at
    // least one sample point in the range where value > 145.
    assert!(!ids.is_empty(), "expected ≥1 passing id");
    let layout = space.layout().clone();
    for id in &ids {
        let baseline = layout.extract(*id, "baseline");
        assert!((130..=140).contains(&baseline));
        // Independently verify at 100 uniform samples.
        let mut found = false;
        let range_ns = (end - start).num_nanoseconds().unwrap();
        for k in 0..100 {
            let t = start + Duration::nanoseconds(range_ns * k / 100);
            let volatility = layout.extract(*id, "volatility") as f64;
            let v = baseline as f64 + oscillate(*id, t, Duration::days(1), 5.0 * volatility, 0.0);
            if v > 145.0 {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "id {id} was yielded but no sample crosses 145 on verify"
        );
    }
}

#[test]
fn forall_in_range_yields_only_ids_that_always_exceed_threshold() {
    let space = people_space();
    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap(); // 1 day
    let layout = space.layout().clone();

    // Constant value = baseline (no oscillation). Forall baseline > 100 →
    // ids with baseline ∈ {101..=200} (we pin via where_range).
    let ids: Vec<u64> = space
        .find()
        .where_range("baseline", 150, 160)
        .at_any(start, end)
        .filter_forall_in_range(
            "baseline_gt_100",
            move |id, _t| layout.extract(id, "baseline") as f64,
            Op::Gt,
            100.0,
            StabilityMode::BinarySearch { probes: 4 },
        )
        .execute()
        .collect();

    // baseline ∈ [150, 160] is 11 values × 16 volatility = 176 ids, all pass.
    assert_eq!(ids.len(), 176);
}

#[test]
fn temporal_range_budget_caps_evaluations() {
    let space = people_space();
    let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();

    let mut results = space
        .find()
        .where_range("baseline", 0, 255)
        .at_any(start, end)
        .scan_budget(50)
        .filter_exists_in_range(
            "never",
            |_id, _t| 0.0,
            Op::Gt,
            1.0,
            StabilityMode::BinarySearch { probes: 2 },
        )
        .execute();
    let _: Vec<u64> = (&mut results).collect();

    assert_eq!(results.evaluated(), 50);
    assert_eq!(
        results.termination(),
        &procedural_core::search::Termination::Budgeted { evaluated: 50 }
    );
}
