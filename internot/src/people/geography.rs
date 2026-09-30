//! Geography subsystem — birth city + current city + relocation
//! history.
//!
//! Until now, every person's location was their cohort city
//! (immutable from the BitLayout). Real people grow up in one
//! place and often move. This module derives:
//!
//! - `birth_city_of(person_id)` — the city they grew up in.
//!   Same country as the cohort, possibly different city. ~70%
//!   of persons are born in their cohort city ("never moved");
//!   30% have a different birth city.
//! - `current_city_of_at(person_id, t)` — at any time `t`,
//!   the city they currently live in. Defaults to cohort city
//!   from age 18 onward; can be overridden by relocation events
//!   (currently always returns the cohort city — relocation
//!   timeline is in-progress).
//! - `years_in_current_city_of_at(person_id, t)` — how long
//!   they've been living there. Anchored on a Tier 4 hash so
//!   different persons have different "settled" durations.
//!
//! All purely derived from `(career_arc_seed, country_code)` —
//! no new substrate bits.

use chrono::{DateTime, Utc};
use procedural_core::hash::hash_int;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use crate::universe::DEFAULT_NOW;

use super::derive::{age_of, country_idx_of};
use super::slot::career_arc_seed_of;
use super::tables::CITIES;

/// Probability a person was born in their cohort city (i.e. never
/// moved). 70% by default — matches US Census ~30% have moved across
/// county lines in their lifetime, but inflated for international
/// realism (substrate models adults; movers self-select into modeled
/// jobs).
const SAME_BIRTH_CITY_RATE: u64 = 70;

/// City this person was born in (same country as cohort). Returns
/// the city's name in their country's local language (the existing
/// CITIES table).
pub fn birth_city_of(person_id: U512) -> &'static str {
    let country = country_idx_of(person_id);
    let city_table = CITIES[country % CITIES.len()];
    let seed = career_arc_seed_of(person_id) as u64;
    let same = hash_int(seed, "geography_same_birth_city_v1", 100) < SAME_BIRTH_CITY_RATE;

    let city_idx = if same {
        // Born in cohort city.
        let cohort_city = super::slot::city_idx_of(person_id) as usize;
        cohort_city % city_table.len()
    } else {
        // Born in a different city of the same country. Hash-pick
        // ANY of the 16 cities; if it collides with cohort, bump.
        let cohort_city = super::slot::city_idx_of(person_id) as usize % city_table.len();
        let pick = hash_int(seed, "geography_birth_city_v1", city_table.len() as u64) as usize;
        if pick == cohort_city {
            (pick + 1) % city_table.len()
        } else {
            pick
        }
    };
    city_table[city_idx]
}

/// Current city of this person at time `t`. v1 always returns the
/// cohort city (same as `derive::city_of`); the relocation timeline
/// is a v2 extension. Kept as a separate `_at` accessor so consumers
/// can switch transparently when relocations land.
pub fn current_city_of_at(person_id: U512, _t: DateTime<Utc>) -> &'static str {
    super::derive::city_of(person_id)
}

/// Years this person has been in their current city. Derived from a
/// hash-anchored "settle" date capped at `age - 18` (you can't have
/// lived in a city longer than you've been an adult).
pub fn years_in_current_city_of_at(person_id: U512, t: DateTime<Utc>) -> u32 {
    let cohort_city = super::slot::city_idx_of(person_id);
    let birth_city = birth_city_of(person_id);
    let cohort_city_name = super::derive::city_of(person_id);

    if birth_city == cohort_city_name {
        // Never moved — they've been there their whole adult life.
        return age_of(person_id).saturating_sub(17);
    }

    // Moved at some point. Hash-derive the move age in [18, age].
    let age_today = age_at_time(person_id, t);
    if age_today < 18 {
        return 0;
    }
    let seed = career_arc_seed_of(person_id) as u64;
    let upper = (age_today - 18).max(1) as u64;
    let move_age_offset = hash_int(seed, "geography_move_age_v1", upper);
    let move_age = 18 + move_age_offset as u32;
    let _ = cohort_city; // hold the var alive — used in shadow above
    age_today.saturating_sub(move_age)
}

fn age_at_time(person_id: U512, t: DateTime<Utc>) -> u32 {
    super::slot::age_at(person_id, t)
}

/// Compact location snapshot for renderers / AVMs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationSnapshot {
    pub birth_city: String,
    pub current_city: String,
    pub years_in_current_city: u32,
    /// True iff `birth_city == current_city`.
    pub never_moved: bool,
}

pub fn location_snapshot_of(person_id: U512) -> LocationSnapshot {
    location_snapshot_of_at(person_id, DEFAULT_NOW())
}

pub fn location_snapshot_of_at(person_id: U512, t: DateTime<Utc>) -> LocationSnapshot {
    let birth = birth_city_of(person_id).to_string();
    let current = current_city_of_at(person_id, t).to_string();
    let never_moved = birth == current;
    LocationSnapshot {
        birth_city: birth,
        current_city: current,
        years_in_current_city: years_in_current_city_of_at(person_id, t),
        never_moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::{person_for, person_id_for};

    fn pid(industry: u8, city: u8, ws: u8, member: u16) -> U512 {
        person_id_for(person_for(industry, city, ws, member))
    }

    #[test]
    fn birth_city_is_in_same_country_as_cohort() {
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let country = country_idx_of(p);
            let table = CITIES[country % CITIES.len()];
            let birth = birth_city_of(p);
            assert!(
                table.contains(&birth),
                "birth city {birth} not in country {country} table"
            );
        }
    }

    #[test]
    fn birth_city_distribution_matches_70_30() {
        let mut same = 0u32;
        let n = 2_000u32;
        for raw in 0..n {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            if birth_city_of(p) == super::super::derive::city_of(p) {
                same += 1;
            }
        }
        let frac = same as f64 / n as f64;
        // SAME_BIRTH_CITY_RATE = 70%; allow ±5pp.
        assert!(
            (0.65..=0.75).contains(&frac),
            "same-city rate {frac} out of [0.65, 0.75]"
        );
    }

    #[test]
    fn never_moved_implies_years_equals_age_minus_17() {
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let snap = location_snapshot_of(p);
            if snap.never_moved {
                let expected = super::super::derive::age_of(p).saturating_sub(17);
                assert_eq!(
                    snap.years_in_current_city, expected,
                    "never-moved person mid={mid:#x} years={} but expected {expected}",
                    snap.years_in_current_city
                );
            }
        }
    }

    #[test]
    fn moved_persons_have_years_less_than_age_minus_17() {
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let snap = location_snapshot_of(p);
            if !snap.never_moved {
                let max_possible = super::super::derive::age_of(p).saturating_sub(17);
                assert!(
                    snap.years_in_current_city <= max_possible,
                    "moved person years {} > max possible {max_possible}",
                    snap.years_in_current_city
                );
            }
        }
    }

    #[test]
    fn snapshot_is_deterministic() {
        let p = pid(0, 0, 0, 7);
        let a = location_snapshot_of(p);
        let b = location_snapshot_of(p);
        assert_eq!(a.birth_city, b.birth_city);
        assert_eq!(a.current_city, b.current_city);
        assert_eq!(a.years_in_current_city, b.years_in_current_city);
    }
}
