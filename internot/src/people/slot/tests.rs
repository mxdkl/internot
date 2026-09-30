//! Integration-style tests for the people slot.
//!
//! These exercise the end-to-end coherence story: BitLayout shape,
//! mail_id ↔ U512 round-trip, populate determinism, T1 cache vs T2
//! source-of-truth invariants, T3 graph-stub round-trips, T4
//! lifecycle round-trips, and Tier 2 facet/domain consistency.

use procedural_core::word::{BitWord, U512};

use crate::universe::DEFAULT_NOW;

use super::convert::*;
use super::labels::archetype_label;
use super::layout::*;
use super::temporal::*;
use super::tier1::*;
use super::tier2::*;
use super::tier3::*;
use super::tier4::*;

#[test]
fn layout_total_width_is_32() {
    assert_eq!(people_layout().total_width(), 32);
}

#[test]
fn person_for_returns_u32_mail_id() {
    let mid: u32 = person_for(5, 3, 17, 42);
    assert_eq!(industry_idx_of_mail_id(mid), 5);
    assert_eq!(city_idx_of_mail_id(mid), 3);
    assert_eq!(workplace_seed_of_mail_id(mid), 17);
    assert_eq!(member_idx_of_mail_id(mid), 42);
}

#[test]
fn person_id_for_yields_mail_id_in_bottom_32_bits() {
    let mid: u32 = person_for(7, 4, 8, 100);
    let pid: U512 = person_id_for(mid);
    assert_eq!(mail_id_of(pid), mid);
    assert_eq!(pid.extract_bits(0, 32) as u32, mid);
}

#[test]
fn populate_extended_is_deterministic() {
    let mid: u32 = person_for(3, 2, 1, 7);
    assert_eq!(person_id_for(mid), person_id_for(mid));
}

#[test]
fn population_size_is_2_to_the_32() {
    assert_eq!(POPULATION_SIZE, 1u64 << 32);
}

#[test]
fn t1_derived_attrs_extract_into_documented_widths() {
    let mid: u32 = person_for(5, 3, 17, 42);
    let pid: U512 = person_id_for(mid);
    assert!(archetype_of(pid) < (1 << T1_WIDTH_ARCHETYPE));
    assert!(cognitive_band_of(pid) < (1 << T1_WIDTH_COGNITIVE));
    assert!(engagement_band_of(pid) < (1 << T1_WIDTH_ENGAGEMENT));
    assert!(values_quadrant_of(pid) < (1 << T1_WIDTH_VALUES));
    assert!(risk_band_of(pid) < (1 << T1_WIDTH_RISK));
    assert!(lifecycle_phase_of(pid) < (1 << T1_WIDTH_LIFECYCLE));
    assert!(network_position_of(pid) < (1 << T1_WIDTH_NETWORK));
    assert!(honesty_band_of(pid) < (1 << T1_WIDTH_HONESTY));
    assert!(self_monitor_of(pid) < (1 << T1_WIDTH_SELF_MONITOR));
    let _ = archetype_label(archetype_of(pid));
}

#[test]
fn build_space_registers_all_four_bit_fields() {
    let s = build_people_space();
    let names = s.attribute_names();
    for f in ["industry_idx", "city_idx", "workplace_seed", "member_idx"] {
        assert!(names.contains(&f.to_string()), "missing field: {}", f);
    }
}

// ---------- Tier 2 archetype-domain consistency ----------

#[test]
fn t2_domains_respect_archetype_bands() {
    for raw in 0..1000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let arch = archetype_of(pid);
        // Binary OCEA: archetype bit = 1 → value ≥ 128; bit = 0 → value < 128
        for (bit, getter, name) in [
            (0u8, openness_of as fn(U512) -> u8, "O"),
            (1, conscientiousness_of, "C"),
            (2, extraversion_of, "E"),
            (3, agreeableness_of, "A"),
        ] {
            let expected_high = (arch & (1 << bit)) != 0;
            let value = getter(pid);
            if expected_high {
                assert!(value >= 128, "mid={mid:#x} arch={arch} {name}={value} expected ≥128 (high band)");
            } else {
                assert!(value < 128, "mid={mid:#x} arch={arch} {name}={value} expected <128 (low band)");
            }
        }
        // 4-level N: archetype bits 4-5 give band; value should be in (band*64)..(band*64+64).
        let n_band = ((arch >> 4) & 3) as u32;
        let n_value = neuroticism_of(pid) as u32;
        let lo = n_band * 64;
        let hi = lo + 64;
        assert!(
            (lo..hi).contains(&n_value),
            "mid={mid:#x} arch={arch} N band={n_band} value={n_value} out of [{lo}, {hi})"
        );
    }
}

// ---------- Tier 2 Phase 2C consistency rules ----------

#[test]
fn t1_values_quadrant_equals_argmax_of_schwartz() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let cached = values_quadrant_of(pid);
        let vals = [
            schwartz_self_transcendence_of(pid),
            schwartz_self_enhancement_of(pid),
            schwartz_conservation_of(pid),
            schwartz_openness_to_change_of(pid),
        ];
        let mut argmax = 0u8;
        for i in 1..4 {
            if vals[i as usize] > vals[argmax as usize] {
                argmax = i;
            }
        }
        assert_eq!(cached, argmax, "mid={mid:#x} cache={cached} argmax={argmax}");
    }
}

#[test]
fn t1_risk_band_equals_risk_tolerance_div16() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        assert_eq!(risk_band_of(pid), risk_tolerance_of(pid) >> 4);
    }
}

#[test]
fn t1_honesty_band_equals_hexaco_h_div16() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        assert_eq!(honesty_band_of(pid), hexaco_honesty_of(pid) >> 4);
    }
}

#[test]
fn t1_dark_flag_matches_threshold_on_psy_plus_mach() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let psy = dt_psychopathy_of(pid) as u32;
        let mach = dt_machiavellianism_of(pid) as u32;
        let expected = (psy + mach) >= T1_DARK_FLAG_THRESHOLD;
        assert_eq!(dark_flag_of(pid), expected, "mid={mid:#x} psy={psy} mach={mach}");
    }
}

#[test]
fn t1_self_monitor_matches_continuous_high_half() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let cont = self_monitor_continuous_of(pid);
        let expected = if cont >= 8 { 1 } else { 0 };
        assert_eq!(self_monitor_of(pid), expected, "mid={mid:#x} cont={cont}");
    }
}

#[test]
fn dark_flag_distribution_stays_near_top_decile() {
    let mut on = 0u32;
    let n = 10_000u32;
    for raw in 0..n {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        if dark_flag_of(pid) {
            on += 1;
        }
    }
    let frac = on as f64 / n as f64;
    assert!(
        (0.07..=0.15).contains(&frac),
        "dark_flag rate {} out of expected ~0.10 band", frac
    );
}

#[test]
fn schwartz_quadrants_are_distinct_per_person() {
    let mut all_distinct = 0u32;
    let n = 1_000u32;
    for raw in 0..n {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let vals = [
            schwartz_self_transcendence_of(pid),
            schwartz_self_enhancement_of(pid),
            schwartz_conservation_of(pid),
            schwartz_openness_to_change_of(pid),
        ];
        let unique: std::collections::HashSet<_> = vals.iter().collect();
        if unique.len() == 4 {
            all_distinct += 1;
        }
    }
    let frac = all_distinct as f64 / n as f64;
    assert!(frac >= 0.80, "only {frac} of persons had 4 distinct Schwartz values");
}

// ---------- Tier 3 graph-stub round-trips ----------

#[test]
fn t3_neighbor_recovery_is_inverse_of_xor_encoding() {
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let spouse = spouse_mail_id_of(pid);
        assert_eq!(spouse ^ spouse_xor_of(pid), mid);
        let mgr = manager_mail_id_of(pid);
        assert_eq!(mgr ^ manager_xor_of(pid), mid);
        let mentor = mentor_mail_id_of(pid);
        assert_eq!(mentor ^ mentor_xor_of(pid), mid);
        let collab = frequent_collab_mail_id_of(pid);
        assert_eq!(collab ^ frequent_collab_xor_of(pid), mid);
    }
}

#[test]
fn manager_lives_in_same_workplace() {
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let mgr = manager_mail_id_of(pid);
        assert_eq!(
            mid & 0xFFFF_F000,
            mgr & 0xFFFF_F000,
            "mid={mid:#x} mgr={mgr:#x} differ in workplace upper bits"
        );
        assert_ne!(
            mid & 0xFFF,
            mgr & 0xFFF,
            "mid={mid:#x} mgr={mgr:#x} have same member_idx (no self-management)"
        );
    }
}

#[test]
fn mentor_shares_industry_but_not_city() {
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let mentor = mentor_mail_id_of(pid);
        assert_eq!(
            (mid >> 26) & 0x3F,
            (mentor >> 26) & 0x3F,
            "mid={mid:#x} mentor={mentor:#x} differ in industry"
        );
        assert_ne!(
            (mid >> 20) & 0x3F,
            (mentor >> 20) & 0x3F,
            "mid={mid:#x} mentor={mentor:#x} same city (mentor should be remote)"
        );
    }
}

#[test]
fn collab_in_same_workplace_distinct_from_self_and_manager() {
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let mgr = manager_mail_id_of(pid);
        let collab = frequent_collab_mail_id_of(pid);
        assert_eq!(mid & 0xFFFF_F000, collab & 0xFFFF_F000);
        assert_ne!(mid & 0xFFF, collab & 0xFFF);
        assert_ne!(mgr & 0xFFF, collab & 0xFFF);
    }
}

// ---------- Tier 4 lifecycle round-trips ----------

#[test]
fn cached_lifecycle_phase_matches_temporal_function_at_default_now() {
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let cached = lifecycle_phase_of(pid);
        let derived = lifecycle_phase_at(pid, DEFAULT_NOW());
        assert_eq!(
            cached, derived,
            "mid={mid:#x} cached={cached} derived={derived}"
        );
    }
}

#[test]
fn age_at_default_now_matches_age_of() {
    use crate::people::derive::age_of;
    for raw in 0..2_000u32 {
        let mid = raw.wrapping_mul(0x1_000_193);
        let pid = person_id_for(mid);
        let from_age_at = age_at(pid, DEFAULT_NOW());
        let from_age_of = age_of(pid);
        assert_eq!(
            from_age_at, from_age_of,
            "mid={mid:#x}: age_at={from_age_at} age_of={from_age_of}"
        );
    }
}

#[test]
fn age_at_is_non_decreasing_in_time() {
    let later = DEFAULT_NOW() + chrono::Duration::days(365 * 5);
    for raw in 0..200u32 {
        let mid = raw.wrapping_mul(0xDEAD_BEEF);
        let pid = person_id_for(mid);
        let now_age = age_at(pid, DEFAULT_NOW());
        let later_age = age_at(pid, later);
        assert!(
            later_age >= now_age,
            "mid={mid:#x}: now_age={now_age} later_age={later_age}"
        );
    }
}

#[test]
fn birth_date_round_trips_through_lifecycle_epoch() {
    for raw in 0..50u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let epoch_days = lifecycle_epoch_of(pid) as i64;
        let bd = birth_date_of(pid);
        let expected = simulation_epoch().date_naive() + chrono::Duration::days(epoch_days);
        assert_eq!(bd, expected, "mid={mid:#x}");
    }
}

#[test]
fn population_age_distribution_stays_in_documented_range() {
    use crate::people::derive::{age_of, MAX_AGE, MIN_AGE};
    for raw in 0..5_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let a = age_of(pid);
        assert!(
            (MIN_AGE..=MAX_AGE).contains(&a),
            "age {a} out of [{MIN_AGE}, {MAX_AGE}] for mid={mid:#x}"
        );
    }
}

#[test]
fn lifecycle_phase_for_age_band_boundaries() {
    assert_eq!(lifecycle_phase_for_age(18), 0);
    assert_eq!(lifecycle_phase_for_age(21), 0);
    assert_eq!(lifecycle_phase_for_age(22), 1);
    assert_eq!(lifecycle_phase_for_age(29), 1);
    assert_eq!(lifecycle_phase_for_age(30), 2);
    assert_eq!(lifecycle_phase_for_age(40), 3);
    assert_eq!(lifecycle_phase_for_age(50), 4);
    assert_eq!(lifecycle_phase_for_age(60), 5);
    assert_eq!(lifecycle_phase_for_age(68), 6);
    assert_eq!(lifecycle_phase_for_age(76), 7);
    assert_eq!(lifecycle_phase_for_age(120), 7);
}

#[test]
fn career_arc_and_life_event_seeds_are_distinct() {
    let mut equal_count = 0u32;
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        if career_arc_seed_of(pid) == life_event_timeline_seed_of(pid) {
            equal_count += 1;
        }
    }
    assert!(equal_count <= 1, "career_arc and life_event seeds collided {equal_count} times in 1K samples");
}

// ---------- Tier 2 facet-domain consistency ----------

#[test]
fn t2_facets_track_archetype_bands() {
    type G = fn(U512) -> u8;
    let ocea_groups: [(u8, [G; 3], &str); 4] = [
        (1u8, [o_aesthetics_of as G, o_intellect_of as G, o_imagination_of as G], "O"),
        (2,   [c_organization_of, c_productiveness_of, c_responsibility_of], "C"),
        (4,   [e_sociability_of, e_assertiveness_of, e_energy_of], "E"),
        (8,   [a_compassion_of, a_respect_of, a_trust_of], "A"),
    ];
    for raw in 0..1_000u32 {
        let mid = raw.wrapping_mul(0x9E37_79B9);
        let pid = person_id_for(mid);
        let arch = archetype_of(pid);
        for (bit, getters, name) in ocea_groups {
            let high = (arch & bit) != 0;
            for g in getters {
                let f = g(pid);
                if high {
                    assert!(f >= 8, "mid={mid:#x} arch={arch} {name}+ but facet={f}");
                } else {
                    assert!(f < 8, "mid={mid:#x} arch={arch} {name}- but facet={f}");
                }
            }
        }
        // N: only assert extreme bands (0 = clearly low, 3 = clearly high).
        let n_band = (arch >> 4) & 3;
        let n_facets = [n_anxiety_of(pid), n_depression_of(pid), n_volatility_of(pid)];
        if n_band == 0 {
            for f in n_facets {
                assert!(f < 8, "mid={mid:#x} N band 0 but facet={f}");
            }
        } else if n_band == 3 {
            for f in n_facets {
                assert!(f >= 8, "mid={mid:#x} N band 3 but facet={f}");
            }
        }
    }
}

#[test]
fn t2_facets_vary_within_parent_domain() {
    let mut differing = 0u32;
    for raw in 0..200u32 {
        let mid = raw.wrapping_mul(0x1_000_193);
        let pid = person_id_for(mid);
        let a = c_organization_of(pid);
        let b = c_productiveness_of(pid);
        let c = c_responsibility_of(pid);
        if a != b || b != c || a != c {
            differing += 1;
        }
    }
    assert!(differing > 50, "facets are too lock-step: {differing}/200 differed");
}

#[test]
fn t2_domains_vary_within_archetype_band() {
    let mut by_arch: std::collections::HashMap<u8, Vec<u32>> =
        std::collections::HashMap::new();
    for raw in 0..2000u32 {
        let mid = raw.wrapping_mul(0x1_000_193);
        let pid = person_id_for(mid);
        by_arch.entry(archetype_of(pid)).or_default().push(mid);
    }
    let pair = by_arch.values().find(|v| v.len() >= 2).expect("≥2 same-arch");
    let a = person_id_for(pair[0]);
    let b = person_id_for(pair[1]);
    let differs = openness_of(a) != openness_of(b)
        || conscientiousness_of(a) != conscientiousness_of(b)
        || extraversion_of(a) != extraversion_of(b)
        || agreeableness_of(a) != agreeableness_of(b)
        || neuroticism_of(a) != neuroticism_of(b);
    assert!(differs, "two same-archetype persons had identical OCEAN values");
}
