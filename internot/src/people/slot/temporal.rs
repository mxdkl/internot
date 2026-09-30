//! Tier 4 temporal anchor + lifecycle phase helpers.
//!
//! `simulation_epoch` (1900-01-01) is the fixed reference point all
//! `lifecycle_epoch` values count from. With 32 bits of day offset
//! the substrate has ~32K years of runway — any plausible birth date
//! 1900..3000 fits.
//!
//! `derive_lifecycle_epoch(mail_id)` is the inverse-of-age formula:
//! given a hash-derived age, compute the day-of-birth offset such
//! that `age_at(id, DEFAULT_NOW)` returns that age exactly.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use procedural_core::hash::hash_int;
use procedural_core::word::{BitWord, U512};

use crate::universe::DEFAULT_NOW;

use super::layout::{T4_OFFSET_LIFECYCLE_EPOCH, T4_WIDTH_LIFECYCLE_EPOCH};

#[inline]
pub fn simulation_epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(1900, 1, 1, 0, 0, 0)
        .single()
        .expect("1900-01-01 is unambiguous in UTC")
}

#[inline]
pub fn days_since_simulation_epoch(t: DateTime<Utc>) -> i64 {
    (t - simulation_epoch()).num_days()
}

/// Hash-derive a (lifecycle_epoch) for a given mail_id. The age
/// distribution mirrors the previous `derive::age_of` formula
/// (8 deca-buckets + 0..6 within-bucket spread) so existing scenarios
/// see the same shape. The returned value is the day-of-birth offset
/// (from SIMULATION_EPOCH) that, evaluated at DEFAULT_NOW, returns
/// the same age the hash drew.
pub fn derive_lifecycle_epoch(mail_id: u32) -> u64 {
    const BUCKETS: [u32; 8] = [18, 22, 28, 35, 45, 55, 65, 75];
    let m = mail_id as u64;
    let bucket = hash_int(m, "age_decade_v2", BUCKETS.len() as u64) as usize;
    let within = hash_int(m, "age_within_bucket", 6) as u32;
    let age_years = BUCKETS[bucket].saturating_add(within);
    let within_year_days = hash_int(m, "lifecycle_within_year_days_v1", 365) as i64;
    let now_days = days_since_simulation_epoch(DEFAULT_NOW());
    // Age in days, using 365-day years to keep the inverse simple.
    // The Tier-1 `age_at` formula uses the same 365-day approximation
    // so the round-trip is exact (no leap-year drift).
    let age_days = (age_years as i64) * 365 + within_year_days;
    (now_days - age_days).max(0) as u64
}

/// Birth date for this person — recovered from `lifecycle_epoch_of`.
pub fn birth_date_of(person_id: U512) -> NaiveDate {
    let days = person_id.extract_bits(T4_OFFSET_LIFECYCLE_EPOCH, T4_WIDTH_LIFECYCLE_EPOCH) as i64;
    let epoch = simulation_epoch().date_naive();
    epoch + chrono::Duration::days(days)
}

/// Age of this person at time `t`, in whole years (365-day years
/// to keep the inverse-of-derive_lifecycle_epoch round-trip exact).
/// Returns 0 if `t` is before the person's birth.
pub fn age_at(person_id: U512, t: DateTime<Utc>) -> u32 {
    let birth_days =
        person_id.extract_bits(T4_OFFSET_LIFECYCLE_EPOCH, T4_WIDTH_LIFECYCLE_EPOCH) as i64;
    let t_days = days_since_simulation_epoch(t);
    let age_days = (t_days - birth_days).max(0);
    (age_days / 365) as u32
}

/// Map an age (in years) to a lifecycle phase 0..7. Bands are sized
/// to give roughly-uniform population coverage across the 18..80 age
/// range the rest of the substrate uses.
pub fn lifecycle_phase_for_age(age: u32) -> u8 {
    match age {
        0..=21 => 0,  // student / early-twenties
        22..=29 => 1, // early-career
        30..=39 => 2, // mid-career
        40..=49 => 3, // peak-career
        50..=59 => 4, // late-career
        60..=67 => 5, // semi-retired
        68..=75 => 6, // retired
        _ => 7,       // long-retired
    }
}

/// Lifecycle phase at time `t` — derived from this person's
/// `lifecycle_epoch` (Tier 4 seed). Matches the cached Tier 1
/// `lifecycle_phase_of(id)` exactly when `t == DEFAULT_NOW()`.
#[inline]
pub fn lifecycle_phase_at(person_id: U512, t: DateTime<Utc>) -> u8 {
    lifecycle_phase_for_age(age_at(person_id, t))
}
