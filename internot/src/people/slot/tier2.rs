//! Tier 2 continuous trait accessors (bits 64..256 of person_id).
//!
//! Big Five domains + 15 facets, Schwartz 4-quadrant values,
//! behavioral economics (risk tolerance / time discount / prosocial),
//! attachment (ECR-R 2D), Dark Triad (SD3), CSE, HEXACO Honesty-
//! Humility, and continuous self-monitor.
//!
//! See `populate.rs` for the centroid+wiggle generation and the
//! coherence invariants linking T1 cached bands to T2 sources.

use procedural_core::word::{BitWord, U512};

use super::layout::*;

// ---------- Big Five domains (8-bit, 0..255) ----------

#[inline]
pub fn openness_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_OPENNESS, T2_WIDTH_BIG_FIVE_DOMAIN) as u8
}

#[inline]
pub fn conscientiousness_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_CONSCIENTIOUSNESS, T2_WIDTH_BIG_FIVE_DOMAIN) as u8
}

#[inline]
pub fn extraversion_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_EXTRAVERSION, T2_WIDTH_BIG_FIVE_DOMAIN) as u8
}

#[inline]
pub fn agreeableness_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_AGREEABLENESS, T2_WIDTH_BIG_FIVE_DOMAIN) as u8
}

#[inline]
pub fn neuroticism_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_NEUROTICISM, T2_WIDTH_BIG_FIVE_DOMAIN) as u8
}

// ---------- Big Five facets (4-bit, 0..15) ----------

#[inline] pub fn o_aesthetics_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_O_AESTHETICS, T2_WIDTH_FACET) as u8
}
#[inline] pub fn o_intellect_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_O_INTELLECT, T2_WIDTH_FACET) as u8
}
#[inline] pub fn o_imagination_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_O_IMAGINATION, T2_WIDTH_FACET) as u8
}

#[inline] pub fn c_organization_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_C_ORGANIZATION, T2_WIDTH_FACET) as u8
}
#[inline] pub fn c_productiveness_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_C_PRODUCTIVENESS, T2_WIDTH_FACET) as u8
}
#[inline] pub fn c_responsibility_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_C_RESPONSIBILITY, T2_WIDTH_FACET) as u8
}

#[inline] pub fn e_sociability_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_E_SOCIABILITY, T2_WIDTH_FACET) as u8
}
#[inline] pub fn e_assertiveness_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_E_ASSERTIVENESS, T2_WIDTH_FACET) as u8
}
#[inline] pub fn e_energy_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_E_ENERGY, T2_WIDTH_FACET) as u8
}

#[inline] pub fn a_compassion_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_A_COMPASSION, T2_WIDTH_FACET) as u8
}
#[inline] pub fn a_respect_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_A_RESPECT, T2_WIDTH_FACET) as u8
}
#[inline] pub fn a_trust_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_A_TRUST, T2_WIDTH_FACET) as u8
}

#[inline] pub fn n_anxiety_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_N_ANXIETY, T2_WIDTH_FACET) as u8
}
#[inline] pub fn n_depression_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_N_DEPRESSION, T2_WIDTH_FACET) as u8
}
#[inline] pub fn n_volatility_of(p: U512) -> u8 {
    p.extract_bits(T2_OFFSET_N_VOLATILITY, T2_WIDTH_FACET) as u8
}

// ---------- Schwartz 4 higher-order values (6-bit) ----------

#[inline]
pub fn schwartz_self_transcendence_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_SCHWARTZ_SELF_TRANSCENDENCE, T2_WIDTH_SCHWARTZ) as u8
}
#[inline]
pub fn schwartz_self_enhancement_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_SCHWARTZ_SELF_ENHANCEMENT, T2_WIDTH_SCHWARTZ) as u8
}
#[inline]
pub fn schwartz_conservation_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_SCHWARTZ_CONSERVATION, T2_WIDTH_SCHWARTZ) as u8
}
#[inline]
pub fn schwartz_openness_to_change_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_SCHWARTZ_OPENNESS_TO_CHANGE, T2_WIDTH_SCHWARTZ) as u8
}

// ---------- Behavioral economics (6-bit) ----------

#[inline]
pub fn risk_tolerance_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_RISK_TOLERANCE, T2_WIDTH_BEH_ECON) as u8
}
#[inline]
pub fn time_discount_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_TIME_DISCOUNT, T2_WIDTH_BEH_ECON) as u8
}
#[inline]
pub fn prosocial_orientation_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_PROSOCIAL, T2_WIDTH_BEH_ECON) as u8
}

// ---------- Attachment (ECR-R 2D, 6-bit) ----------

#[inline]
pub fn attachment_anxiety_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_ATTACHMENT_ANXIETY, T2_WIDTH_ATTACHMENT) as u8
}
#[inline]
pub fn attachment_avoidance_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_ATTACHMENT_AVOIDANCE, T2_WIDTH_ATTACHMENT) as u8
}

// ---------- Dark Triad / SD3 (4-bit) ----------

#[inline]
pub fn dt_psychopathy_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_DT_PSYCHOPATHY, T2_WIDTH_DARK_TRIAD) as u8
}
#[inline]
pub fn dt_machiavellianism_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_DT_MACHIAVELLIANISM, T2_WIDTH_DARK_TRIAD) as u8
}
#[inline]
pub fn dt_narcissism_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_DT_NARCISSISM, T2_WIDTH_DARK_TRIAD) as u8
}

// ---------- CSE / HEXACO H / continuous self-monitor ----------

#[inline]
pub fn cse_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_CSE, T2_WIDTH_CSE) as u8
}
#[inline]
pub fn hexaco_honesty_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_HEXACO_H, T2_WIDTH_HEXACO_H) as u8
}
#[inline]
pub fn self_monitor_continuous_of(person_id: U512) -> u8 {
    person_id.extract_bits(T2_OFFSET_SELF_MONITOR_C, T2_WIDTH_SELF_MONITOR_C) as u8
}
