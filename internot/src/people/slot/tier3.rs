//! Tier 3 graph-stub accessors (bits 256..384 of person_id).
//!
//! Each stub stores `neighbor_mail_id XOR self_mail_id`. Recovering
//! the neighbor is `stub ^ self_low32`. Constraints (same-workplace
//! manager, same-industry mentor, etc.) are enforced at populate-time.

use procedural_core::word::{BitWord, U512};

use super::convert::mail_id_of;
use super::layout::*;

// ---------- Raw XOR-encoded stubs ----------

#[inline]
pub fn spouse_xor_of(person_id: U512) -> u32 {
    person_id.extract_bits(T3_OFFSET_SPOUSE_XOR, T3_WIDTH_NEIGHBOR_XOR) as u32
}
#[inline]
pub fn manager_xor_of(person_id: U512) -> u32 {
    person_id.extract_bits(T3_OFFSET_MANAGER_XOR, T3_WIDTH_NEIGHBOR_XOR) as u32
}
#[inline]
pub fn mentor_xor_of(person_id: U512) -> u32 {
    person_id.extract_bits(T3_OFFSET_MENTOR_XOR, T3_WIDTH_NEIGHBOR_XOR) as u32
}
#[inline]
pub fn frequent_collab_xor_of(person_id: U512) -> u32 {
    person_id.extract_bits(T3_OFFSET_FREQUENT_COLLAB_XOR, T3_WIDTH_NEIGHBOR_XOR) as u32
}

// ---------- Decoded neighbor mail_ids ----------

#[inline]
pub fn spouse_mail_id_of(person_id: U512) -> u32 {
    spouse_xor_of(person_id) ^ mail_id_of(person_id)
}
#[inline]
pub fn manager_mail_id_of(person_id: U512) -> u32 {
    manager_xor_of(person_id) ^ mail_id_of(person_id)
}
#[inline]
pub fn mentor_mail_id_of(person_id: U512) -> u32 {
    mentor_xor_of(person_id) ^ mail_id_of(person_id)
}
#[inline]
pub fn frequent_collab_mail_id_of(person_id: U512) -> u32 {
    frequent_collab_xor_of(person_id) ^ mail_id_of(person_id)
}
