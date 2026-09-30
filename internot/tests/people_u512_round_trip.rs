//! Integration test: People moved to World<U512>, but cross-service
//! references (mail_id u32) and PersonAvm semantics are unchanged.
//! Validates the spec invariants from
//! docs/superpowers/specs/2026-05-04-person-512bit-substrate.md.

use internot::people::{
    archetype_of, cognitive_band_of, dark_flag_of, honesty_band_of, industry_idx_of_mail_id,
    lifecycle_phase_of, mail_id_of, network_position_of, person_for, person_id_for, risk_band_of,
    self_monitor_of, values_quadrant_of, PersonAvm,
};

#[test]
fn person_id_for_round_trips_through_mail_id() {
    let mid = person_for(7, 4, 8, 100);
    let pid = person_id_for(mid);
    assert_eq!(mail_id_of(pid), mid);
}

#[test]
fn person_avm_constructed_from_u32_mail_id() {
    let mid = person_for(7, 4, 8, 100);
    let avm = PersonAvm::for_mail_id(mid);
    assert_eq!(avm.mail_id, mid);
    assert!(!avm.handle.is_empty());
    assert!(!avm.country.is_empty());
}

#[test]
fn t1_derived_attrs_surface_on_avm() {
    let mid = person_for(7, 4, 8, 100);
    let avm = PersonAvm::for_mail_id(mid);
    // Every T1 derived attr is present and within its declared range.
    assert!(avm.personality_archetype < 64);
    assert!(avm.cognitive_band < 4);
    assert!(avm.values_quadrant < 4);
    assert!(avm.risk_band < 4);
    assert!(avm.lifecycle_phase < 8);
    assert!(avm.network_position < 4);
    assert!(avm.honesty_band < 4);
    assert!(avm.self_monitor < 2);
    // dark_flag is bool — no range check needed
}

#[test]
fn t1_derived_attrs_consistent_with_slot_extractors() {
    let mid = person_for(3, 2, 1, 7);
    let pid = person_id_for(mid);
    let avm = PersonAvm::for_mail_id(mid);
    assert_eq!(avm.personality_archetype, archetype_of(pid));
    assert_eq!(avm.cognitive_band, cognitive_band_of(pid));
    assert_eq!(avm.values_quadrant, values_quadrant_of(pid));
    assert_eq!(avm.risk_band, risk_band_of(pid));
    assert_eq!(avm.lifecycle_phase, lifecycle_phase_of(pid));
    assert_eq!(avm.network_position, network_position_of(pid));
    assert_eq!(avm.honesty_band, honesty_band_of(pid));
    assert_eq!(avm.dark_flag, dark_flag_of(pid));
    assert_eq!(avm.self_monitor, self_monitor_of(pid));
}

#[test]
fn cross_service_mail_id_unchanged() {
    let mid: u32 = person_for(5, 3, 17, 42);
    assert_eq!(industry_idx_of_mail_id(mid), 5);
}
