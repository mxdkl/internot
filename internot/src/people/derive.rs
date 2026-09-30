//! Hash-derived attributes on `person_id`. Pure functions of the slot.
//!
//! Per the bit-vs-hash search rule (learning.md): bit fields are for
//! finding, hashes are for enriching. Country, age, gender, name,
//! voice all live here as descriptive read-only attributes — they
//! cannot be used as a primary search axis.

use procedural_core::hash::{hash_float, hash_int};
use procedural_core::word::U512;

use super::names::{first_name_for, last_name_for, Gender};
use super::slot::{city_idx_of, industry_idx_of, mail_id_of};
use super::tables::{CITIES, COUNTRIES, COUNTRY_CODES, INDUSTRIES};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullName {
    pub first: &'static str,
    pub last: &'static str,
}

impl std::fmt::Display for FullName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.first, self.last)
    }
}

/// Which row of `COUNTRIES` / `COUNTRY_CODES` / `CITIES` this person
/// belongs to. Hash-derived from the WORKPLACE bits (industry + city_idx
/// + workplace_seed = upper 20 bits of mail_id), not from member_idx —
/// because a workplace is in a specific country, and all members of
/// the same workplace must therefore agree on country/city/timezone.
/// Keying on the full mail_id (the previous v2 behavior) gave members
/// of the same workplace different countries, which broke
/// list_people_in_workplace coherence and Tier 3 graph traversal
/// (manager / collab were "in the same workplace by bits" but read
/// out as living in a different country than the employee).
pub fn country_idx_of(person_id: U512) -> usize {
    let workplace_bits = mail_id_of(person_id) & 0xFFFF_F000;
    hash_int(workplace_bits as u64, "country_v3", COUNTRIES.len() as u64) as usize
}

pub fn country_of(person_id: U512) -> &'static str {
    COUNTRIES[country_idx_of(person_id)]
}

pub fn country_code_of(person_id: U512) -> &'static str {
    COUNTRY_CODES[country_idx_of(person_id)]
}

/// IANA timezone name for this person, derived from their country.
/// Multi-timezone countries map to their most-populous zone (US →
/// America/New_York). For per-city resolution we'd need IANA boundary
/// data — overkill for the current scenario surface.
pub fn timezone_of(person_id: U512) -> &'static str {
    super::timezones::tz_of(country_code_of(person_id))
}

/// Raw UTC offset in minutes for this person's country, ignoring DST.
/// Use for working-hours / quiet-hours derivations: a person at
/// offset_minutes=60 (e.g. Italy) has working hours 9-17 local =
/// 8-16 UTC.
pub fn utc_offset_minutes_of(person_id: U512) -> i32 {
    super::timezones::offset_minutes_of(country_code_of(person_id))
}

/// City — looked up against the country derived above. The 6-bit
/// `city_idx` field from the slot picks the row within the country.
pub fn city_of(person_id: U512) -> &'static str {
    let row = &CITIES[country_idx_of(person_id) % CITIES.len()];
    row[(city_idx_of(person_id) as usize) % row.len()]
}

pub fn industry_of(person_id: U512) -> &'static str {
    INDUSTRIES[(industry_idx_of(person_id) as usize) % INDUSTRIES.len()]
}

pub fn gender_of(person_id: U512) -> Gender {
    if hash_int(mail_id_of(person_id) as u64, "gender_v1", 2) == 0 {
        Gender::Male
    } else {
        Gender::Female
    }
}

/// Family id — hash-bucketed into 65536 families. Two persons with
/// the same `family_id_of` are siblings (share a last name); finding
/// a sibling requires either knowing both ids in advance or a sweep.
/// Family is not enumerable — that's the architectural rule.
pub fn family_id_of(person_id: U512) -> u128 {
    hash_int(mail_id_of(person_id) as u64, "family_id_v2", 65536) as u128
}

/// First name from the procedural names dataset, scoped to
/// `(country_code, gender)`. Falls back to a deterministic synthetic
/// if the locale isn't in the dataset.
pub fn first_name_of(person_id: U512) -> &'static str {
    let cc = country_code_of(person_id);
    let g = gender_of(person_id);
    let seed = hash_int(mail_id_of(person_id) as u64, "first_name_idx_v3", u64::MAX);
    first_name_for(cc, g, seed).unwrap_or(synthetic_first_name(seed))
}

/// Last name — keyed on `family_id_of` (not `person_id`) so siblings
/// share a surname. Locale picked from `family_birth_country_code`
/// (also keyed on family_id) so siblings also share locale heritage.
pub fn last_name_of(person_id: U512) -> &'static str {
    let family = family_id_of(person_id);
    let cc_idx = hash_int(family as u64, "family_birth_country_code", COUNTRIES.len() as u64) as usize;
    let cc = COUNTRY_CODES[cc_idx];
    let seed = hash_int(family as u64, "last_name_idx_v3", u64::MAX);
    last_name_for(cc, seed).unwrap_or(synthetic_last_name(seed))
}

pub fn full_name_of(person_id: U512) -> FullName {
    FullName {
        first: first_name_of(person_id),
        last: last_name_of(person_id),
    }
}

/// `<firstname>_<l><suffix>` style handle — deterministic per person.
/// Multi-word first names ("Jorge Alejandro") collapse to the first
/// token to keep handles single-word and URL-safe.
pub fn handle_of(person_id: U512) -> String {
    let name = full_name_of(person_id);
    let suffix = hash_int(mail_id_of(person_id) as u64, "handle_suffix", 100);
    let first_token = name
        .first
        .split_whitespace()
        .next()
        .unwrap_or(name.first);
    let last_token = name
        .last
        .split_whitespace()
        .next()
        .unwrap_or(name.last);
    let initial = last_token.chars().next().unwrap_or('x').to_ascii_lowercase();
    format!("{}_{}{:02}", first_token.to_ascii_lowercase(), initial, suffix)
}

/// Age in years at the canonical `DEFAULT_NOW`. Now driven by the
/// person's Tier 4 `lifecycle_epoch` seed (single source of truth) —
/// the resulting distribution is identical to the previous direct hash
/// because `derive_lifecycle_epoch` re-uses the same age-bucket formula
/// when computing the birth date.
pub fn age_of(person_id: U512) -> u32 {
    use crate::universe::DEFAULT_NOW;
    super::slot::age_at(person_id, DEFAULT_NOW())
}

pub const MIN_AGE: u32 = 18;
pub const MAX_AGE: u32 = 80;

/// Voice descriptor — formality / verbosity / vocabulary baseline.
/// Used by mail/calendar renderers to keep a person sounding the same
/// across messages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceDescriptor {
    pub formality_baseline: f32,
    pub verbosity: f32,
    pub vocabulary: VocabularyRegister,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabularyRegister {
    Casual,
    Neutral,
    Formal,
    Technical,
}

pub fn voice_of(person_id: U512) -> VoiceDescriptor {
    let f = hash_float(mail_id_of(person_id) as u64, "voice_formality") as f32;
    let v = hash_float(mail_id_of(person_id) as u64, "voice_verbosity") as f32;
    let r = hash_int(mail_id_of(person_id) as u64, "voice_vocab", 4);
    let vocab = match r {
        0 => VocabularyRegister::Casual,
        1 => VocabularyRegister::Neutral,
        2 => VocabularyRegister::Formal,
        _ => VocabularyRegister::Technical,
    };
    VoiceDescriptor {
        formality_baseline: f,
        verbosity: v,
        vocabulary: vocab,
    }
}

/// Engagement style — how actively a person posts to chats / social.
/// A categorical primitive matching real-platform 80/20: most
/// people are Lurkers, a tiny tail Influencers.
///
/// **Future direction:** when the people slot widens past 32 bits
/// (Layer 1 widening), this should be promoted to a 2-bit indexable
/// field so cohort queries like "all influencers in r/rust's
/// people-city" can pushdown via `Space::find()`. The hash-derived
/// implementation here keeps the API stable across that migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum EngagementStyle {
    /// ~50% of population: rarely posts. 0–3 posts/year typical.
    Lurker,
    /// ~35%: comments occasionally, posts a handful per year.
    Casual,
    /// ~12%: regular contributor, dozens of posts per year.
    Contributor,
    /// ~3%: high-volume poster, can dominate small communities.
    Influencer,
}

impl EngagementStyle {
    /// Average posts per year for this style. Used as the location
    /// parameter in the per-author lognormal post-count sampler.
    pub fn baseline_posts_per_year(self) -> f64 {
        match self {
            EngagementStyle::Lurker      =>   1.0,
            EngagementStyle::Casual      =>  10.0,
            EngagementStyle::Contributor => 100.0,
            EngagementStyle::Influencer  => 800.0,
        }
    }
}

/// Reads the cached Tier 1 engagement band (which `populate_extended`
/// derives via `categorical(mail_id, ...)` with the documented 50/35/12/3
/// weights). Sourcing from the cache instead of re-deriving via a U512
/// hash means the AVM and the cached band can never disagree — a bug
/// the `find_people` view surfaced when the U512-keyed `categorical`
/// drifted from the u64-keyed `populate_extended`.
pub fn engagement_style_of(person_id: U512) -> EngagementStyle {
    match super::slot::engagement_band_of(person_id) {
        0 => EngagementStyle::Lurker,
        1 => EngagementStyle::Casual,
        2 => EngagementStyle::Contributor,
        _ => EngagementStyle::Influencer,
    }
}

// --- synthetic fallbacks (only hit if a locale isn't in the dataset) ---

const SYNTHETIC_FIRST: &[&str] = &["Alex", "Sam", "Jordan", "Taylor", "Riley", "Morgan"];
const SYNTHETIC_LAST: &[&str] = &["Smith", "Lee", "Patel", "Garcia", "Khan", "Müller"];

fn synthetic_first_name(seed: u64) -> &'static str {
    SYNTHETIC_FIRST[(seed as usize) % SYNTHETIC_FIRST.len()]
}

fn synthetic_last_name(seed: u64) -> &'static str {
    SYNTHETIC_LAST[(seed as usize) % SYNTHETIC_LAST.len()]
}

#[cfg(test)]
mod tests {
    use super::super::slot::{person_for, person_id_for};
    use super::*;

    #[test]
    fn full_name_is_deterministic() {
        let mid = person_for(5, 3, 17, 42);
        let id = person_id_for(mid);
        assert_eq!(full_name_of(id), full_name_of(id));
    }

    #[test]
    fn last_name_matches_family_keyed_lookup() {
        // Two persons with the same family_id should share a last name.
        // Birthday paradox in 65536 bins means a collision is very
        // likely well under 5K samples.
        use std::collections::HashMap;
        let mut by_family: HashMap<u128, U512> = HashMap::new();
        for raw in 0..5_000u32 {
            let p = person_id_for(raw);
            let fam = family_id_of(p);
            if let Some(prev) = by_family.get(&fam) {
                assert_eq!(last_name_of(*prev), last_name_of(p));
                return;
            }
            by_family.insert(fam, p);
        }
        panic!("no same-family pair within 5K samples (extremely unlikely)");
    }

    #[test]
    fn age_in_documented_range() {
        for raw in 0..1000u32 {
            let id = person_id_for(raw.wrapping_mul(0xdead_beef));
            let a = age_of(id);
            assert!((MIN_AGE..=MAX_AGE).contains(&a), "age {} out of range", a);
        }
    }

    #[test]
    fn voice_components_in_unit_range() {
        for raw in 0..200u32 {
            let id = person_id_for(raw.wrapping_mul(0x1234));
            let v = voice_of(id);
            assert!((0.0..=1.0).contains(&v.formality_baseline));
            assert!((0.0..=1.0).contains(&v.verbosity));
        }
    }

    #[test]
    fn engagement_style_distribution_is_heavy_tailed() {
        // Sample 10K people; the population should be ~50/35/12/3 lurker
        // /casual/contributor/influencer. Generous tolerances since 10K
        // samples on a 4-bin categorical have noticeable variance.
        let mut counts = [0u32; 4];
        for raw in 0..10_000u32 {
            let id = person_id_for(raw.wrapping_mul(0x9E37_79B9));
            let bin = match engagement_style_of(id) {
                EngagementStyle::Lurker      => 0,
                EngagementStyle::Casual      => 1,
                EngagementStyle::Contributor => 2,
                EngagementStyle::Influencer  => 3,
            };
            counts[bin] += 1;
        }
        // Very loose bounds — just verify the order is right.
        assert!(counts[0] > counts[1], "more lurkers than casuals");
        assert!(counts[1] > counts[2], "more casuals than contributors");
        assert!(counts[2] > counts[3], "more contributors than influencers");
        assert!(counts[3] > 100, "≥1% influencers");
    }

    #[test]
    fn engagement_style_is_deterministic() {
        let mid = super::super::slot::person_for(5, 3, 17, 42);
        let id = person_id_for(mid);
        assert_eq!(engagement_style_of(id), engagement_style_of(id));
    }

    #[test]
    fn handle_starts_with_lowercase_first_name() {
        let mid = person_for(2, 4, 8, 16);
        let id = person_id_for(mid);
        let name = full_name_of(id);
        let h = handle_of(id);
        let first_token = name.first.split_whitespace().next().unwrap();
        assert!(h.starts_with(&first_token.to_ascii_lowercase()));
    }

    #[test]
    fn handles_never_contain_whitespace() {
        // Multi-word first/last names ("Jorge Alejandro", "Martin Muñoz")
        // must collapse to single-token handles. Sweep enough of the
        // population to hit at least one multi-word locale (the dataset
        // contains plenty of these, especially for Spanish-speaking
        // countries and compound surnames).
        for raw in 0..2_000u32 {
            let id = person_id_for(raw.wrapping_mul(0x9E37_79B9));
            let h = handle_of(id);
            assert!(
                !h.chars().any(|c| c.is_whitespace()),
                "handle contains whitespace: {h:?} (mail_id={raw})"
            );
        }
    }
}
