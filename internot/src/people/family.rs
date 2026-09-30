//! Family graph — promotes Tier 3 / life-events partner & child
//! references from "seeds" to actual `mail_id`s of real persons in
//! the population (where possible).
//!
//! Why this matters: with `spouse_mail_id_of(pid)` returning a
//! random Tier 3 candidate, Eli's "spouse" might be 80 years old
//! and live in a different country. Not realistic. This module
//! constraint-searches for a compatible match.
//!
//! ## v1 design — "adults-only population, minors as stubs"
//!
//! Per the substrate's MIN_AGE=18 invariant (the population models
//! adults who use online services), we can't promote a 5-year-old
//! child to a real PersonAvm. Instead `FamilyChild` is a sum type:
//!
//! - `FamilyChild::Adult { mail_id }` — full PersonAvm available.
//!   Used for adult children of older parents.
//! - `FamilyChild::Minor { first_name, last_name, age, gender }` —
//!   not in the population substrate; only carries the renderable
//!   identity. Kids in profiles, not agents.
//!
//! The user has flagged "model all ages" as a follow-up; the
//! substrate-level lower-MIN_AGE work is deferred (would need to
//! gate every service by age).
//!
//! ## v1 limitations (documented honestly)
//!
//! - **One-way references.** If A's spouse_compat resolves to B,
//!   B's spouse_compat does NOT necessarily resolve to A. Reciprocal
//!   marriages need a pair-symmetric pairing algorithm; that's a v2
//!   substrate problem. Renderers / scenarios that require
//!   reciprocity should check `b.spouse_mail_id == a.mail_id`.
//! - **Heteronormative spouse search** (opposite-gender match). Same
//!   reasoning as the v1 stub — gender-preference modelling is a
//!   future extension.
//!
//! All searches are cached so the brute-force loop runs at most once
//! per (input) per process.

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use procedural_core::hash::hash_int;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use crate::universe::DEFAULT_NOW;

use super::derive::{country_code_of, gender_of, last_name_of};
use super::names::{first_name_for, Gender};
use super::slot::{age_at, mail_id_of, person_id_for, spouse_xor_of};

const SPOUSE_AGE_TOLERANCE: u32 = 10;
const CHILD_AGE_TOLERANCE: u32 = 2;
/// Parents are typically born 22–45 years before their child.
const PARENT_AGE_GAP_MIN: u32 = 22;
const PARENT_AGE_GAP_MAX: u32 = 45;
const SEARCH_TRIALS: u64 = 256;

/// Resolved child of a parent — either a real adult in the population
/// (≥ 18 today) or a stub for a minor child.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FamilyChild {
    /// Minor child — not in the population substrate; carries only
    /// the renderable identity (name + age + gender) derived
    /// deterministically from the parent + child_index.
    Minor {
        first_name: String,
        last_name: String,
        age: u32,
        gender: String,
    },
    /// Adult child — full PersonAvm available via `mail_id`.
    Adult {
        mail_id: u32,
        age: u32,
    },
}

impl FamilyChild {
    /// First name accessor that works for both variants — for
    /// renderers that just want "what's the kid's name."
    pub fn first_name(&self) -> String {
        match self {
            FamilyChild::Minor { first_name, .. } => first_name.clone(),
            FamilyChild::Adult { mail_id, .. } => {
                super::derive::first_name_of(person_id_for(*mail_id)).to_string()
            }
        }
    }

    pub fn age(&self) -> u32 {
        match self {
            FamilyChild::Minor { age, .. } => *age,
            FamilyChild::Adult { age, .. } => *age,
        }
    }
}

// ---------- Caches ----------

fn spouse_cache() -> &'static RwLock<HashMap<U512, u32>> {
    static C: OnceLock<RwLock<HashMap<U512, u32>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

fn child_cache() -> &'static RwLock<HashMap<(u32, u8, u32), FamilyChild>> {
    static C: OnceLock<RwLock<HashMap<(u32, u8, u32), FamilyChild>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

/// `(person_mail_id, parent_index 0=mother | 1=father) → parent mail_id`
fn parent_cache() -> &'static RwLock<HashMap<(u32, u8), Option<u32>>> {
    static C: OnceLock<RwLock<HashMap<(u32, u8), Option<u32>>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

// ---------- Spouse ----------

/// A spouse who's a real person in the population, with age within
/// `SPOUSE_AGE_TOLERANCE` years of the person and opposite gender.
///
/// Walks candidates derived from the Tier 3 `spouse_xor_of` seed,
/// accepting the first that matches. After `SEARCH_TRIALS` failed
/// trials, returns the raw Tier 3 spouse — graceful degradation;
/// at least every person has *some* spouse.
pub fn spouse_mail_id_compat_at(person_id: U512, t: DateTime<Utc>) -> u32 {
    if let Some(hit) = spouse_cache().read().get(&person_id).copied() {
        return hit;
    }

    let person_age = age_at(person_id, t);
    let person_gender = gender_of(person_id);
    let person_mail_id = mail_id_of(person_id);
    let seed = spouse_xor_of(person_id) as u64;

    for trial in 0..SEARCH_TRIALS {
        let probe_seed = seed.wrapping_add(trial.wrapping_mul(0x9E37_79B9));
        let candidate_mail_id = hash_int(probe_seed, "spouse_search_v1", 1u64 << 32) as u32;
        if candidate_mail_id == person_mail_id {
            continue;
        }
        let candidate_pid = person_id_for(candidate_mail_id);
        let candidate_age = age_at(candidate_pid, t);
        if person_age.abs_diff(candidate_age) > SPOUSE_AGE_TOLERANCE {
            continue;
        }
        if gender_of(candidate_pid) == person_gender {
            // Heteronormative match for v1 — see module docs.
            continue;
        }
        spouse_cache().write().insert(person_id, candidate_mail_id);
        return candidate_mail_id;
    }

    let fallback = super::slot::spouse_mail_id_of(person_id);
    spouse_cache().write().insert(person_id, fallback);
    fallback
}

#[inline]
pub fn spouse_mail_id_compat(person_id: U512) -> u32 {
    spouse_mail_id_compat_at(person_id, DEFAULT_NOW())
}

// ---------- Children ----------

/// Resolve a child of `parent_mail_id` at `child_index` whose age
/// today (relative to DEFAULT_NOW) is `target_age_today`.
///
/// - If `target_age_today >= 18`, searches the adult population
///   for an age-matched person and returns `FamilyChild::Adult`.
/// - Otherwise, synthesizes a `FamilyChild::Minor` stub with name +
///   gender derived deterministically from (parent, child_index).
///   Last name == parent's last name (basic family-name coherence).
pub fn child_at(parent_mail_id: u32, child_index: u8, target_age_today: u32) -> FamilyChild {
    let key = (parent_mail_id, child_index, target_age_today);
    if let Some(hit) = child_cache().read().get(&key).cloned() {
        return hit;
    }

    let resolved = if target_age_today >= 18 {
        resolve_adult_child(parent_mail_id, child_index, target_age_today)
    } else {
        resolve_minor_child(parent_mail_id, child_index, target_age_today)
    };
    child_cache().write().insert(key, resolved.clone());
    resolved
}

fn resolve_adult_child(parent_mail_id: u32, child_index: u8, target_age: u32) -> FamilyChild {
    let seed = (parent_mail_id as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((child_index as u64) << 32);

    for trial in 0..SEARCH_TRIALS {
        let probe_seed = seed.wrapping_add(trial.wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
        let candidate_mail_id = hash_int(probe_seed, "child_search_v1", 1u64 << 32) as u32;
        if candidate_mail_id == parent_mail_id {
            continue;
        }
        let candidate_age = age_at(person_id_for(candidate_mail_id), DEFAULT_NOW());
        if candidate_age.abs_diff(target_age) <= CHILD_AGE_TOLERANCE {
            return FamilyChild::Adult {
                mail_id: candidate_mail_id,
                age: candidate_age,
            };
        }
    }
    // Fallback — return a hash-derived id even if age mismatches. The
    // resulting AVM may have a different age than `target_age`; callers
    // should prefer FamilyChild::age() over recomputing from the AVM.
    let fallback = hash_int(seed, "child_fallback_v1", 1u64 << 32) as u32;
    let actual_age = age_at(person_id_for(fallback), DEFAULT_NOW());
    FamilyChild::Adult { mail_id: fallback, age: actual_age }
}

fn resolve_minor_child(parent_mail_id: u32, child_index: u8, target_age: u32) -> FamilyChild {
    let parent_pid = person_id_for(parent_mail_id);
    let parent_country = country_code_of(parent_pid);
    let parent_last_name = last_name_of(parent_pid).to_string();

    let seed = (parent_mail_id as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((child_index as u64) << 32);

    let gender_pick = hash_int(seed, "child_gender_v1", 2);
    let gender = if gender_pick == 0 { Gender::Female } else { Gender::Male };

    // Use a large name-table index — first_name_for masks against the
    // table size internally.
    let name_idx = hash_int(seed, "child_first_name_v1", 1u64 << 32);
    let first_name = first_name_for(parent_country, gender, name_idx)
        .unwrap_or("Alex")
        .to_string();

    FamilyChild::Minor {
        first_name,
        last_name: parent_last_name,
        age: target_age,
        gender: match gender {
            Gender::Male => "male".to_string(),
            Gender::Female => "female".to_string(),
        },
    }
}

// ---------- Parents ----------

/// A real person in the population who's age-compatible to be this
/// person's parent. `parent_index`: 0 = mother (Female), 1 = father
/// (Male). Returns None if the parent would be older than MAX_AGE
/// (in that case, the parent has likely passed away in the
/// substrate's age window — modelable as bereavement in v2).
///
/// Search constraints: age = person_age + [PARENT_AGE_GAP_MIN..=MAX]
/// and matching gender. Same-search-trials structure as spouse_compat.
pub fn parent_mail_id_of(person_id: U512, parent_index: u8) -> Option<u32> {
    let person_mail_id = mail_id_of(person_id);
    let key = (person_mail_id, parent_index);
    if let Some(hit) = parent_cache().read().get(&key).copied() {
        return hit;
    }
    let resolved = resolve_parent(person_id, parent_index);
    parent_cache().write().insert(key, resolved);
    resolved
}

fn resolve_parent(person_id: U512, parent_index: u8) -> Option<u32> {
    use super::derive::{age_of, country_code_of, MAX_AGE};

    let person_age = age_of(person_id);
    let min_parent_age = person_age.saturating_add(PARENT_AGE_GAP_MIN);
    let max_parent_age = person_age.saturating_add(PARENT_AGE_GAP_MAX).min(MAX_AGE);
    if min_parent_age > max_parent_age {
        return None;
    }
    let want_gender = if parent_index == 0 { Gender::Female } else { Gender::Male };
    let person_mail_id = mail_id_of(person_id);
    let person_country = country_code_of(person_id);
    let seed = (person_mail_id as u64)
        .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        .wrapping_add(parent_index as u64);

    // Two-pass: first try same country (realism), then any country
    // (graceful degradation — some seeds have no same-country candidate
    // in 256 trials given the ~6% per-country marginal probability).
    let mut fallback: Option<u32> = None;
    for trial in 0..SEARCH_TRIALS {
        let probe_seed = seed.wrapping_add(trial.wrapping_mul(0x9E37_79B9));
        let candidate_mid = hash_int(probe_seed, "parent_search_v1", 1u64 << 32) as u32;
        if candidate_mid == person_mail_id {
            continue;
        }
        let candidate_pid = person_id_for(candidate_mid);
        let candidate_age = age_of(candidate_pid);
        if candidate_age < min_parent_age || candidate_age > max_parent_age {
            continue;
        }
        if gender_of(candidate_pid) != want_gender {
            continue;
        }
        if country_code_of(candidate_pid) == person_country {
            return Some(candidate_mid);
        }
        if fallback.is_none() {
            fallback = Some(candidate_mid);
        }
    }
    fallback
}

// ---------- Reciprocal marriage ----------

/// Returns Some(spouse_mail_id) iff the marriage is reciprocal —
/// i.e., A's spouse_compat = B AND B's spouse_compat = A. None if
/// the link is one-way (most pairs in v1).
pub fn reciprocal_spouse_of(person_id: U512) -> Option<u32> {
    let candidate = spouse_mail_id_compat(person_id);
    let candidate_pid = person_id_for(candidate);
    let back = spouse_mail_id_compat(candidate_pid);
    if back == mail_id_of(person_id) {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::derive::age_of;
    use crate::people::{person_for, person_id_for};

    #[test]
    fn spouse_age_within_tolerance() {
        let mut checked = 0u32;
        let mut failures = 0u32;
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let person_age = age_of(pid);
            let spouse_mid = spouse_mail_id_compat(pid);
            let spouse_age = age_of(person_id_for(spouse_mid));
            checked += 1;
            if person_age.abs_diff(spouse_age) > SPOUSE_AGE_TOLERANCE {
                failures += 1;
            }
        }
        let fail_rate = failures as f64 / checked as f64;
        assert!(
            fail_rate < 0.05,
            "spouse age mismatch rate {fail_rate} > 5% (failures={failures}/{checked})"
        );
    }

    #[test]
    fn spouse_is_not_self() {
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            assert_ne!(spouse_mail_id_compat(pid), mid);
        }
    }

    #[test]
    fn spouse_is_deterministic() {
        let pid = person_id_for(person_for(8, 0, 1, 7));
        assert_eq!(spouse_mail_id_compat(pid), spouse_mail_id_compat(pid));
    }

    #[test]
    fn spouse_is_opposite_gender_in_majority() {
        let mut opp = 0u32;
        let mut total = 0u32;
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let person_g = gender_of(pid);
            let spouse_pid = person_id_for(spouse_mail_id_compat(pid));
            total += 1;
            if gender_of(spouse_pid) != person_g {
                opp += 1;
            }
        }
        let rate = opp as f64 / total as f64;
        assert!(rate >= 0.95, "opposite-gender rate {rate} < 95%");
    }

    #[test]
    fn adult_child_age_close_to_target() {
        let parent = person_for(8, 0, 1, 7);
        for child_index in 0..3u8 {
            for target in [20u32, 25, 30, 35] {
                let child = child_at(parent, child_index, target);
                match child {
                    FamilyChild::Adult { mail_id, age } => {
                        assert!(
                            age.abs_diff(target) <= CHILD_AGE_TOLERANCE,
                            "adult child age {age} not within ±{CHILD_AGE_TOLERANCE} of {target}"
                        );
                        assert_ne!(mail_id, parent);
                    }
                    FamilyChild::Minor { .. } => panic!("expected Adult variant for target {target}"),
                }
            }
        }
    }

    #[test]
    fn minor_child_returns_stub_with_age() {
        let parent = person_for(8, 0, 1, 7);
        for target in [3u32, 8, 12, 17] {
            let child = child_at(parent, 0, target);
            match child {
                FamilyChild::Minor { age, first_name, last_name, .. } => {
                    assert_eq!(age, target);
                    assert!(!first_name.is_empty());
                    assert!(!last_name.is_empty());
                }
                FamilyChild::Adult { .. } => panic!("expected Minor variant for target {target}"),
            }
        }
    }

    #[test]
    fn minor_children_share_parent_last_name() {
        let parent = person_for(8, 0, 1, 7);
        let parent_last = last_name_of(person_id_for(parent));
        let child = child_at(parent, 0, 5);
        if let FamilyChild::Minor { last_name, .. } = child {
            assert_eq!(last_name, parent_last);
        } else {
            panic!("expected Minor");
        }
    }

    #[test]
    fn distinct_minor_children_get_distinct_first_names() {
        let parent = person_for(8, 0, 1, 7);
        let c0 = child_at(parent, 0, 5);
        let c1 = child_at(parent, 1, 7);
        let c2 = child_at(parent, 2, 9);
        let n0 = c0.first_name();
        let n1 = c1.first_name();
        let n2 = c2.first_name();
        // Allow collision pairs but at least one distinct.
        assert!(
            !(n0 == n1 && n1 == n2),
            "all three children got same name: {n0}"
        );
    }
}
