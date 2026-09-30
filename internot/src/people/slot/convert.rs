//! Conversions between the public 32-bit `mail_id`, its individual
//! cohort axes, and the U512 person_id (with cached derived attrs).
//!
//! `person_id_for(mail_id)` is the hot path for cross-service joins:
//! every time a service surfaces a person it goes through this fn,
//! which calls `populate::populate_extended` to fill the U512 cache.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;
use procedural_core::word::{BitWord, U512};

use super::layout::people_layout;
use super::populate::populate_extended;

/// Process-wide memo of `mail_id → fully-populated U512 person_id`.
///
/// `populate_extended` does ~30 hash_int calls + ~30 insert_bits per
/// invocation (~1.8 µs). Every cross-service join (mail/calendar/
/// tasks surfacing a person) materializes the same handful of
/// mail_ids over and over — within a request and across requests.
/// Cache hits drop to ~50 ns (HashMap read + Copy of a 64-byte
/// U512). Pure-function memo: the cached value is the unique correct
/// answer for that mail_id, no invalidation possible.
fn person_id_cache() -> &'static RwLock<HashMap<u32, U512>> {
    static C: OnceLock<RwLock<HashMap<u32, U512>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

// ---------- mail_id factory ----------

/// Compose a 32-bit mail_id from the four cohort axes. Goes through
/// `BitLayout::compose` so the bit positions stay in lock-step with
/// the registered Space (Space::find pushdown reads the same bits
/// compose writes).
pub fn person_for(industry: u8, city: u8, workplace_seed: u8, member_idx: u16) -> u32 {
    let layout = people_layout();
    let id = layout.compose(&[
        ("member_idx", member_idx as u64),
        ("workplace_seed", workplace_seed as u64),
        ("city_idx", city as u64),
        ("industry_idx", industry as u64),
    ]);
    // Bottom 32 bits ARE the mail_id (the layout occupies bits 0..32).
    id.extract_bits(0, 32) as u32
}

// ---------- Extract individual cohort axes from a mail_id ----------
//
// Layout (per `people_layout` LSB→MSB declaration order):
//   bits 0..12   = member_idx
//   bits 12..20  = workplace_seed
//   bits 20..26  = city_idx
//   bits 26..32  = industry_idx

#[inline]
pub fn industry_idx_of_mail_id(mail_id: u32) -> u8 {
    ((mail_id >> 26) & 0x3F) as u8
}

#[inline]
pub fn city_idx_of_mail_id(mail_id: u32) -> u8 {
    ((mail_id >> 20) & 0x3F) as u8
}

#[inline]
pub fn workplace_seed_of_mail_id(mail_id: u32) -> u8 {
    ((mail_id >> 12) & 0xFF) as u8
}

#[inline]
pub fn member_idx_of_mail_id(mail_id: u32) -> u16 {
    (mail_id & 0xFFF) as u16
}

// ---------- Same accessors but reading from a U512 person_id ----------

#[inline]
pub fn industry_idx_of(person_id: U512) -> u8 {
    industry_idx_of_mail_id(mail_id_of(person_id))
}

#[inline]
pub fn city_idx_of(person_id: U512) -> u8 {
    city_idx_of_mail_id(mail_id_of(person_id))
}

#[inline]
pub fn workplace_seed_of(person_id: U512) -> u8 {
    workplace_seed_of_mail_id(mail_id_of(person_id))
}

#[inline]
pub fn member_idx_of(person_id: U512) -> u16 {
    member_idx_of_mail_id(mail_id_of(person_id))
}

// ---------- Mail_id ↔ U512 conversion ----------

/// Extract the 32-bit mail_id from a populated U512 person_id.
#[inline]
pub fn mail_id_of(person_id: U512) -> u32 {
    person_id.extract_bits(0, 32) as u32
}

/// Reconstruct the full U512 person_id from a 32-bit mail_id —
/// populates Tier 1–4 cached attributes via `populate_extended`.
/// Hot path: every cross-service join calls this.
pub fn person_id_for(mail_id: u32) -> U512 {
    // Cache fast-path. RwLock read is cheap; on a hit the U512
    // copy beats re-running ~30 hash_int calls in populate_extended.
    if let Some(hit) = person_id_cache().read().get(&mail_id).copied() {
        return hit;
    }
    // Miss — compute, then take the write lock to insert. A racing
    // miss may compute the same value twice; that's harmless (pure
    // function) and cheaper than holding the write lock during compute.
    let mut id = U512::default().insert_bits(0, 32, mail_id as u64);
    populate_extended(&mut id, mail_id);
    person_id_cache().write().insert(mail_id, id);
    id
}
