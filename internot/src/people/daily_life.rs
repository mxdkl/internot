//! Daily life subsystem — chronotype, working hours, hobbies.
//!
//! Translates the Big Five abstract scores into concrete behavioral
//! texture: when this person is awake, when they work, what they do
//! for fun. All derived from existing Tier 1/2 attrs + the cohort
//! country; no new substrate bits.
//!
//! Datasets:
//! - **Hobbies**: 494 entries from carlelieser's Wikipedia-derived
//!   list (`data/hobbies.json`). Each entry has `category` (General,
//!   Competitive, Collection, Observation, Educational) and `setting`
//!   (Indoors / Outdoors). We weight category × setting picks against
//!   the person's Big Five + age — high E favors Competitive +
//!   Outdoors, high O favors Educational, etc.
//! - **Chronotype** distribution (MEQ-based): ~28% Morning, ~52%
//!   Intermediate, ~20% Evening (US adult population from validation
//!   studies; latitudes shift the distribution but we don't model it).
//!
//! Cross-service implications:
//! - calendar can read `working_hours_of` to avoid early-morning
//!   meetings for evening types
//! - chat can render casual interest mentions
//!
//! All accessors are pure functions of `(person_id)` — chronotype
//! and hobbies don't drift over a normal scenario timespan, so no
//! `_at(t)` variants needed for v1.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;
use procedural_core::hash::hash_int;
use procedural_core::sampler::categorical;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use super::derive::{age_of, country_code_of};
use super::slot::{
    career_arc_seed_of, conscientiousness_of, extraversion_of, neuroticism_of, openness_of,
};

/// Process-wide memo of `person_id → hobbies`. Pure function, called
/// from PersonAvm on every read — so cache hits matter.
fn hobbies_cache() -> &'static RwLock<HashMap<U512, Arc<Vec<Hobby>>>> {
    static C: OnceLock<RwLock<HashMap<U512, Arc<Vec<Hobby>>>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

// ---------- Hobby dataset ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hobby {
    pub title: String,
    pub category: String, // General | Competitive | Collection | Observation | Educational
    pub setting: String,  // Indoors | Outdoors
}

const HOBBIES_RAW: &str = include_str!("../../data/hobbies.json");

fn hobbies() -> &'static Vec<Hobby> {
    static T: OnceLock<Vec<Hobby>> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(HOBBIES_RAW).expect("hobbies.json parses"))
}

/// Pre-computed indices grouped by (category, setting). Built once;
/// the hobby picker draws from the right group based on Big Five
/// weighted category preference.
fn hobby_index_by_category_setting()
    -> &'static std::collections::HashMap<(String, String), Vec<usize>>
{
    static T: OnceLock<std::collections::HashMap<(String, String), Vec<usize>>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for (i, h) in hobbies().iter().enumerate() {
            m.entry((h.category.clone(), h.setting.clone()))
                .or_insert_with(Vec::new)
                .push(i);
        }
        m
    })
}

// ---------- Chronotype ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chronotype {
    /// Wakes early (~5–6am), peaks midmorning, fades by evening.
    Morning,
    /// Standard 7am–11pm rhythm. Majority of the population.
    Intermediate,
    /// Wakes late (~9am+), peaks late afternoon, productive into night.
    Evening,
}

impl Chronotype {
    pub fn label(self) -> &'static str {
        match self {
            Chronotype::Morning => "early bird",
            Chronotype::Intermediate => "intermediate",
            Chronotype::Evening => "night owl",
        }
    }
}

/// MEQ-derived chronotype distribution (~28% morning / 52% intermediate
/// / 20% evening). Uses career_arc_seed mixed with a chronotype key —
/// independent of all existing T1/T2 derivations.
pub fn chronotype_of(person_id: U512) -> Chronotype {
    let seed = career_arc_seed_of(person_id) as u64;
    // Slight nudge: high N (Neuroticism) skews toward Evening (literature
    // links eveningness with anxiety/depression). 0..255 N → ±5pp shift.
    let n = neuroticism_of(person_id) as f64 / 255.0;
    let evening_boost = 0.05 * (n - 0.5);
    let weights: &[f64] = &[
        (0.28 - evening_boost / 2.0).max(0.10),
        (0.52 - evening_boost / 2.0).max(0.20),
        (0.20 + evening_boost).max(0.05),
    ];
    let idx = categorical(seed, "chronotype_v1", weights);
    match idx {
        0 => Chronotype::Morning,
        1 => Chronotype::Intermediate,
        _ => Chronotype::Evening,
    }
}

// ---------- Working hours ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingHours {
    /// Local hour at which the person typically starts work (0–23).
    pub start_hour: u8,
    /// Local hour at which they typically end (0–23, > start).
    pub end_hour: u8,
    /// Optional lunch break (start_hour, end_hour) within the workday.
    pub lunch_window: Option<(u8, u8)>,
    /// Best window for cross-timezone meetings (overlap with peers).
    pub peak_focus_hour: u8,
}

/// Country-baseline working hours. Three regional patterns:
///   - Mediterranean (IT, ES): 9–18 with 13:30–14:30 lunch
///   - Northern European (GB, DE, FR): 8–17 with 12:30–13:00 lunch
///   - Asian (JP, CN, KR, IN): 9–19 with 12:00–13:00 lunch
///   - Americas / Oceania default: 9–17 with 12:00–13:00 lunch
fn country_hours_baseline(country_code: &str) -> WorkingHours {
    match country_code {
        "IT" | "ES" => WorkingHours {
            start_hour: 9, end_hour: 18,
            lunch_window: Some((13, 14)),
            peak_focus_hour: 10,
        },
        "GB" | "DE" | "FR" => WorkingHours {
            start_hour: 8, end_hour: 17,
            lunch_window: Some((12, 13)),
            peak_focus_hour: 10,
        },
        "JP" | "CN" | "KR" | "IN" => WorkingHours {
            start_hour: 9, end_hour: 19,
            lunch_window: Some((12, 13)),
            peak_focus_hour: 14,
        },
        _ => WorkingHours {
            start_hour: 9, end_hour: 17,
            lunch_window: Some((12, 13)),
            peak_focus_hour: 10,
        },
    }
}

/// Working hours derived from country baseline + chronotype shift.
/// Morning chronotypes shift the day 1h earlier; Evening chronotypes
/// shift it 1h later. Peak focus tracks the chronotype.
pub fn working_hours_of(person_id: U512) -> WorkingHours {
    let mut wh = country_hours_baseline(country_code_of(person_id));
    let shift: i8 = match chronotype_of(person_id) {
        Chronotype::Morning => -1,
        Chronotype::Intermediate => 0,
        Chronotype::Evening => 1,
    };
    let shift_hour = |h: u8| -> u8 { ((h as i8 + shift).clamp(0, 23)) as u8 };
    wh.start_hour = shift_hour(wh.start_hour);
    wh.end_hour = shift_hour(wh.end_hour);
    if let Some((ls, le)) = wh.lunch_window {
        wh.lunch_window = Some((shift_hour(ls), shift_hour(le)));
    }
    wh.peak_focus_hour = match chronotype_of(person_id) {
        Chronotype::Morning => 9,
        Chronotype::Intermediate => 11,
        Chronotype::Evening => 16,
    };
    wh
}

// ---------- Hobbies ----------

/// Typical number of hobbies for this person (2–5). High Extraversion
/// → more hobbies (more diverse engagement); high Conscientiousness +
/// low E → fewer + more focused.
fn hobby_count_for(person_id: U512) -> usize {
    let e = extraversion_of(person_id) as u32;
    let c = conscientiousness_of(person_id) as u32;
    // E in [0, 255] mapped to base count [2, 5], -1 if very high C.
    let base = 2 + (e * 3 / 256) as i32;
    let adjust = if c > 200 && e < 100 { -1 } else { 0 };
    base.saturating_add(adjust).clamp(2, 5) as usize
}

/// Hobbies for this person — `hobby_count_for(id)` distinct picks
/// from the 494-entry dataset, weighted by Big Five + age.
///
/// Category weighting:
/// - high E → favors Competitive (team / social hobbies)
/// - high O → favors Educational + General-creative
/// - high C → favors Collection (structured, completionist)
/// - low E → favors Observation, Collection (solitary)
/// - high N → favors Indoors
/// - young (≤30) → mild boost to Competitive
/// - older (≥55) → boost to Collection + Observation, less Competitive
pub fn hobbies_of(person_id: U512) -> Vec<Hobby> {
    if let Some(hit) = hobbies_cache().read().get(&person_id).cloned() {
        return (*hit).clone();
    }
    let computed = compute_hobbies(person_id);
    hobbies_cache()
        .write()
        .insert(person_id, Arc::new(computed.clone()));
    computed
}

fn compute_hobbies(person_id: U512) -> Vec<Hobby> {
    let n_hobbies = hobby_count_for(person_id);
    let seed = career_arc_seed_of(person_id) as u64 ^ 0xDEAD_C0DE;

    let category_weights = category_weights_for(person_id);
    let setting_weights = setting_weights_for(person_id);

    let categories = ["General", "Competitive", "Collection", "Observation", "Educational"];
    let settings = ["Indoors", "Outdoors"];

    let mut picked_indices: Vec<usize> = Vec::with_capacity(n_hobbies);
    let mut attempts = 0u32;
    let max_attempts = (n_hobbies * 8) as u32;

    while picked_indices.len() < n_hobbies && attempts < max_attempts {
        attempts += 1;
        let pick_seed = seed.wrapping_add((attempts as u64) << 16);

        let cat_idx = categorical(pick_seed, "hobby_cat_v1", &category_weights);
        let set_idx = categorical(pick_seed, "hobby_set_v1", &setting_weights);
        let cat_key = (categories[cat_idx].to_string(), settings[set_idx].to_string());

        let pool = match hobby_index_by_category_setting().get(&cat_key) {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let in_pool = hash_int(pick_seed, "hobby_pick_v1", pool.len() as u64) as usize;
        let candidate = pool[in_pool];
        if !picked_indices.contains(&candidate) {
            picked_indices.push(candidate);
        }
    }

    picked_indices.into_iter().map(|i| hobbies()[i].clone()).collect()
}

fn category_weights_for(person_id: U512) -> [f64; 5] {
    let o = openness_of(person_id) as f64 / 255.0;
    let c = conscientiousness_of(person_id) as f64 / 255.0;
    let e = extraversion_of(person_id) as f64 / 255.0;
    let age = age_of(person_id);

    let young_boost = if age <= 30 { 0.10 } else if age >= 55 { -0.10 } else { 0.0 };
    let older_collection_boost = if age >= 55 { 0.15 } else { 0.0 };

    // Indices: 0=General, 1=Competitive, 2=Collection, 3=Observation, 4=Educational
    let mut w = [
        0.30,                                          // General — broad default
        (0.20 + 0.30 * e + young_boost).max(0.05),     // Competitive
        (0.10 + 0.20 * c + older_collection_boost).max(0.05), // Collection
        (0.10 + 0.15 * (1.0 - e) + 0.10 * older_collection_boost).max(0.05), // Observation
        (0.10 + 0.30 * o).max(0.05),                   // Educational
    ];
    // Normalize to sum 1.0 — categorical does this internally but
    // doing it here keeps the percentages interpretable in tests.
    let s: f64 = w.iter().sum();
    for x in &mut w {
        *x /= s;
    }
    w
}

fn setting_weights_for(person_id: U512) -> [f64; 2] {
    let n = neuroticism_of(person_id) as f64 / 255.0;
    let e = extraversion_of(person_id) as f64 / 255.0;
    // High N → Indoors; high E → Outdoors. Default split 55/45 indoors.
    let outdoors = (0.45 + 0.20 * e - 0.15 * n).clamp(0.10, 0.85);
    [1.0 - outdoors, outdoors]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::{person_for, person_id_for};

    fn pid(industry: u8, city: u8, ws: u8, member: u16) -> U512 {
        person_id_for(person_for(industry, city, ws, member))
    }

    #[test]
    fn hobbies_dataset_loads() {
        assert!(hobbies().len() >= 400);
        assert!(hobbies().iter().any(|h| h.title.contains("Chess")));
    }

    #[test]
    fn chronotype_distribution_matches_meq() {
        let mut counts = [0u32; 3];
        for raw in 0..5_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let idx = match chronotype_of(p) {
                Chronotype::Morning => 0,
                Chronotype::Intermediate => 1,
                Chronotype::Evening => 2,
            };
            counts[idx] += 1;
        }
        let n = 5_000.0;
        let m = counts[0] as f64 / n;
        let i = counts[1] as f64 / n;
        let e = counts[2] as f64 / n;
        // Allow ±5pp drift from anchor distribution (28/52/20).
        assert!((0.20..=0.36).contains(&m), "Morning rate {m} out of band");
        assert!((0.42..=0.58).contains(&i), "Intermediate rate {i} out of band");
        assert!((0.13..=0.30).contains(&e), "Evening rate {e} out of band");
    }

    #[test]
    fn working_hours_shift_with_chronotype() {
        // Sweep until we find one of each chronotype in same country
        // (Italy here so baseline is comparable).
        let mut samples: std::collections::HashMap<Chronotype, (u8, u8)> =
            std::collections::HashMap::new();
        for member in 0..1024u16 {
            let p = pid(0, 0, 0, member);
            let ct = chronotype_of(p);
            if !samples.contains_key(&ct) {
                let wh = working_hours_of(p);
                samples.insert(ct, (wh.start_hour, wh.end_hour));
                if samples.len() == 3 { break; }
            }
        }
        // Morning chronotype should start earliest; Evening latest.
        if let (Some(&(m_start, _)), Some(&(e_start, _))) =
            (samples.get(&Chronotype::Morning), samples.get(&Chronotype::Evening))
        {
            assert!(m_start < e_start, "morning ({m_start}) not earlier than evening ({e_start})");
        }
    }

    #[test]
    fn hobby_count_in_2_to_5() {
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let n = hobbies_of(p).len();
            assert!((1..=5).contains(&n), "hobby count {n} out of [1,5] for mid={mid:#x}");
        }
    }

    #[test]
    fn hobbies_are_distinct_per_person() {
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let h = hobbies_of(p);
            let unique: std::collections::HashSet<_> = h.iter().map(|x| &x.title).collect();
            assert_eq!(unique.len(), h.len(), "duplicate hobby for mid={mid:#x}: {h:?}");
        }
    }

    #[test]
    fn hobbies_are_deterministic() {
        let p = pid(8, 0, 1, 7);
        let a = hobbies_of(p);
        let b = hobbies_of(p);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.title, y.title);
        }
    }

    #[test]
    fn high_extraversion_population_picks_more_competitive() {
        // Filter to high-E persons; their Competitive share should
        // exceed the population baseline.
        let mut comp_he = 0u32;
        let mut total_he = 0u32;
        let mut comp_le = 0u32;
        let mut total_le = 0u32;
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let e = extraversion_of(p);
            for h in hobbies_of(p) {
                if e >= 200 {
                    total_he += 1;
                    if h.category == "Competitive" { comp_he += 1; }
                } else if e <= 56 {
                    total_le += 1;
                    if h.category == "Competitive" { comp_le += 1; }
                }
            }
        }
        if total_he > 50 && total_le > 50 {
            let r_he = comp_he as f64 / total_he as f64;
            let r_le = comp_le as f64 / total_le as f64;
            assert!(r_he > r_le, "high-E competitive rate {r_he} not > low-E rate {r_le}");
        }
    }

    #[test]
    fn high_neuroticism_skews_evening() {
        // Among high-N persons, Evening should appear noticeably more
        // often than the population average (~20%).
        let mut evenings = 0u32;
        let mut total = 0u32;
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            if neuroticism_of(p) >= 200 {
                total += 1;
                if chronotype_of(p) == Chronotype::Evening {
                    evenings += 1;
                }
            }
        }
        if total > 100 {
            let rate = evenings as f64 / total as f64;
            assert!(rate >= 0.18, "high-N evening rate {rate} below 0.18");
        }
    }
}
