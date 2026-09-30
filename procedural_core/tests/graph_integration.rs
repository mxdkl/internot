#![allow(deprecated)] // exercises graph::stable_roommates_match until it is removed
//! Cross-cutting integration tests for `procedural_core::graph`.
//! Wires the modules together on a tiny synthetic cohort.

use chrono::{Duration, TimeZone, Utc};

use procedural_core::graph::{
    canonical_pair, comm_intensity, enumerate_events, pair_hash_float, stable_roommates_match,
    tie_strength, PersonalityProjection, Tie, TieStrengthProfile, TIE_KIND_UNSPECIFIED,
};

#[test]
fn end_to_end_household_smoke() {
    // Tiny "cohort" of 4 people. Pair them via stable-roommates with
    // a pair-hash preference. Compute tie strength + intensity +
    // enumerate a month of mail events. Assert non-trivial output.
    // arbitrary distinct ids; the smoke test doesn't depend on values.
    let cohort = vec![100_u32, 200, 300, 400];
    let pairs = stable_roommates_match(&cohort, |a, b| pair_hash_float(a, b, "pref:v1"));
    assert_eq!(pairs.len(), 2, "4-person cohort yields 2 pairs");
    // For the first paired couple:
    let (a, b) = pairs[0];
    let profile = TieStrengthProfile {
        base_floor: 0.1,
        cohabit_peak: 1.0,
        cohabit_start_day: Some(0),
        cohabit_end_day: Some(365 * 30),
        decay_tau_days: 365.0 * 5.0,
    };
    // 10 years into the marriage:
    let s = tie_strength(&profile, 365 * 10);
    assert!(s > 0.95, "cohabiting partner strength near peak; got {s}");
    // Intensity at noon Monday:
    let now = Utc.with_ymd_and_hms(2025, 6, 2, 12, 0, 0).unwrap();
    let p = PersonalityProjection::default();
    let intensity = comm_intensity(s, &p, &p, now);
    assert!(intensity.mail_per_day > 0.0);
    assert!(intensity.chat_per_day > 0.0);
    // Enumerate mail events for the next 30 days.
    let t_start = now;
    let t_end = now + Duration::days(30);
    let ev = enumerate_events(a, b, "mail", |_| intensity.mail_per_day, t_start, t_end);
    assert!(!ev.is_empty(), "expected at least some mail events over a month");
    // Tie + canonical_pair sanity:
    let _tie = Tie {
        peer_id: b,
        kind: TIE_KIND_UNSPECIFIED,
        venue_id: canonical_pair(a, b),
        strength: s,
        since: now,
        last_proc_contact: now,
    };
}

#[test]
fn matching_is_deterministic_across_runs() {
    let cohort = vec![1_u32, 2, 3, 4, 5, 6, 7, 8];
    let r1 = stable_roommates_match(&cohort, |a, b| pair_hash_float(a, b, "pref"));
    let r2 = stable_roommates_match(&cohort, |a, b| pair_hash_float(a, b, "pref"));
    assert_eq!(r1, r2);
}

#[test]
fn event_enumeration_slice_recombinability() {
    // enumerate([t1, t2)) ++ enumerate([t2, t3)) == enumerate([t1, t3)).
    // Holds because every draw is keyed on the absolute day, not the window.
    let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let t_mid = t0 + Duration::days(50);
    let t_end = t0 + Duration::days(100);
    let lambda = |_| 1.0_f64;
    let full = enumerate_events(7, 42, "mail", lambda, t0, t_end);
    let lo = enumerate_events(7, 42, "mail", lambda, t0, t_mid);
    let hi = enumerate_events(7, 42, "mail", lambda, t_mid, t_end);
    let mut combined = lo.clone();
    combined.extend(hi.iter().copied());
    assert_eq!(
        full.iter().map(|e| e.at).collect::<Vec<_>>(),
        combined.iter().map(|e| e.at).collect::<Vec<_>>(),
        "slice-recombinability"
    );
}
