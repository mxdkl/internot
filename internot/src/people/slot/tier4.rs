//! Tier 4 temporal-seed accessors (bits 384..512 of person_id).
//!
//! Each seed is 32 bits. They power the per-person temporal
//! subsystems (lifecycle / career arc / life events). The
//! `lifecycle_epoch` is also the source-of-truth for the cached
//! T1 lifecycle_phase (see `populate.rs`).

use procedural_core::word::{BitWord, U512};

use super::layout::*;

#[inline]
pub fn lifecycle_epoch_of(person_id: U512) -> u32 {
    person_id.extract_bits(T4_OFFSET_LIFECYCLE_EPOCH, T4_WIDTH_LIFECYCLE_EPOCH) as u32
}

#[inline]
pub fn career_arc_seed_of(person_id: U512) -> u32 {
    person_id.extract_bits(T4_OFFSET_CAREER_ARC_SEED, T4_WIDTH_CAREER_ARC_SEED) as u32
}

#[inline]
pub fn life_event_timeline_seed_of(person_id: U512) -> u32 {
    person_id.extract_bits(T4_OFFSET_LIFE_EVENT_SEED, T4_WIDTH_LIFE_EVENT_SEED) as u32
}
