//! Tier 1 derived-attribute accessors (bits 32..64 of person_id).
//!
//! All Tier 1 values are bit-cached during `populate_extended` —
//! these are pure `extract_bits` calls, ~3ns each. The values
//! agree with their source-of-truth functions by construction
//! (see `populate.rs` invariants).

use procedural_core::word::{BitWord, U512};

use super::layout::*;

#[inline]
pub fn archetype_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_ARCHETYPE, T1_WIDTH_ARCHETYPE) as u8
}

#[inline]
pub fn cognitive_band_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_COGNITIVE, T1_WIDTH_COGNITIVE) as u8
}

#[inline]
pub fn engagement_band_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_ENGAGEMENT, T1_WIDTH_ENGAGEMENT) as u8
}

#[inline]
pub fn values_quadrant_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_VALUES, T1_WIDTH_VALUES) as u8
}

#[inline]
pub fn risk_band_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_RISK, T1_WIDTH_RISK) as u8
}

#[inline]
pub fn lifecycle_phase_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_LIFECYCLE, T1_WIDTH_LIFECYCLE) as u8
}

#[inline]
pub fn network_position_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_NETWORK, T1_WIDTH_NETWORK) as u8
}

#[inline]
pub fn honesty_band_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_HONESTY, T1_WIDTH_HONESTY) as u8
}

#[inline]
pub fn dark_flag_of(person_id: U512) -> bool {
    person_id.extract_bits(T1_OFFSET_DARK_FLAG, T1_WIDTH_DARK_FLAG) != 0
}

#[inline]
pub fn self_monitor_of(person_id: U512) -> u8 {
    person_id.extract_bits(T1_OFFSET_SELF_MONITOR, T1_WIDTH_SELF_MONITOR) as u8
}
