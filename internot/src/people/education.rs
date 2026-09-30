//! Education subsystem — degree level + field of study + graduation
//! year, derived per person from `career_arc_seed_of` and the
//! cohort `industry_idx`.
//!
//! Field of study uses the **NCES Classification of Instructional
//! Programs** (CIP) 2-digit families, loaded from
//! `data/cip_families.json` (42 families, e.g. "Computer and
//! Information Sciences", "Engineering", "Health Professions").
//! Per `feedback_realism_first.md` we sourced a real taxonomy
//! rather than inventing one.
//!
//! The field is biased toward the person's cohort industry via
//! `data/naics_to_cip_families.json` — engineers don't materialize
//! in healthcare workplaces. With 80% probability we draw from the
//! industry's curated CIP list; the remaining 20% draws from the
//! full CIP space (cross-industry careers).
//!
//! Education-level distribution (Bachelor's-modal) reflects the US
//! Census attainment for adults 25+:
//!   HighSchool ~35%, Associate ~10%, Bachelors ~25%,
//!   Masters ~15%, PhD ~3%, MD ~1.5%, JD ~0.5%.
//! No-college (HighSchool) covers ~10% no-diploma + ~25% HS-only.
//!
//! Graduation year is derived deterministically from
//! `lifecycle_epoch_of` + a typical-age table per degree.

use std::sync::OnceLock;

use procedural_core::hash::hash_int;
use procedural_core::sampler::categorical;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use super::slot::{birth_date_of, career_arc_seed_of, industry_idx_of};
use super::tables::INDUSTRIES;

// ---------- CIP families dataset ----------

#[derive(Debug, Clone, serde::Deserialize)]
struct CipFamily {
    code: String,
    title: String,
}

const CIP_RAW: &str = include_str!("../../data/cip_families.json");

fn cip_families() -> &'static Vec<CipFamily> {
    static T: OnceLock<Vec<CipFamily>> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(CIP_RAW).expect("cip_families.json parses"))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NaicsToCip {
    mapping: Vec<Vec<String>>,
}

const NAICS_TO_CIP_RAW: &str = include_str!("../../data/naics_to_cip_families.json");

fn naics_to_cip() -> &'static NaicsToCip {
    static T: OnceLock<NaicsToCip> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(NAICS_TO_CIP_RAW).expect("naics_to_cip_families.json parses"))
}

// ---------- Education level ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EducationLevel {
    HighSchool,
    AssociateDegree,
    Bachelors,
    Masters,
    PhD,
    MD,
    JD,
}

impl EducationLevel {
    pub fn label(self) -> &'static str {
        match self {
            EducationLevel::HighSchool => "High School",
            EducationLevel::AssociateDegree => "Associate Degree",
            EducationLevel::Bachelors => "Bachelor's",
            EducationLevel::Masters => "Master's",
            EducationLevel::PhD => "PhD",
            EducationLevel::MD => "MD",
            EducationLevel::JD => "JD",
        }
    }

    /// Typical age at graduation for this level (used to derive
    /// graduation_year from birth date).
    pub fn typical_grad_age(self) -> u32 {
        match self {
            EducationLevel::HighSchool => 18,
            EducationLevel::AssociateDegree => 20,
            EducationLevel::Bachelors => 22,
            EducationLevel::Masters => 24,
            EducationLevel::PhD => 28,
            EducationLevel::MD => 28,
            EducationLevel::JD => 25,
        }
    }
}

const EDUCATION_WEIGHTS: &[f64] = &[
    0.35, // HighSchool
    0.10, // AssociateDegree
    0.25, // Bachelors
    0.15, // Masters
    0.03, // PhD
    0.015, // MD
    0.005, // JD
    // remainder (~10%) bumps Bachelors to ~32% — closer to the actual
    // adult population. The sampler auto-normalizes.
    0.10, // padded onto Bachelors-equivalent (handled below)
];

pub fn education_level_of(person_id: U512) -> EducationLevel {
    let seed = career_arc_seed_of(person_id) as u64;
    let idx = categorical(seed, "education_level_v1", EDUCATION_WEIGHTS);
    match idx {
        0 => EducationLevel::HighSchool,
        1 => EducationLevel::AssociateDegree,
        2 => EducationLevel::Bachelors,
        3 => EducationLevel::Masters,
        4 => EducationLevel::PhD,
        5 => EducationLevel::MD,
        6 => EducationLevel::JD,
        _ => EducationLevel::Bachelors, // padding bucket
    }
}

// ---------- Field of study ----------

/// Field of study for this person — drawn from the CIP 2-digit
/// families table. Biased toward the person's cohort industry 80%
/// of the time; 20% chance of drawing from any field (career
/// switchers, generalists, second careers).
///
/// Returns the CIP family code (e.g. "11" for Computer Science) +
/// human title (e.g. "Computer and Information Sciences and Support
/// Services"). HighSchool-only persons return None — they have no
/// field of study.
pub fn field_of_study_of(person_id: U512) -> Option<FieldOfStudy> {
    if education_level_of(person_id) == EducationLevel::HighSchool {
        return None;
    }
    let seed = career_arc_seed_of(person_id) as u64;
    let industry = industry_idx_of(person_id);

    // 80% industry-aligned, 20% cross-industry generalist.
    let aligned_pick = hash_int(seed, "field_of_study_aligned_v1", 5) < 4;
    let cip_code = if aligned_pick {
        let aligned = naics_to_cip()
            .mapping
            .get(industry as usize)
            .filter(|v| !v.is_empty());
        if let Some(aligned) = aligned {
            let idx = hash_int(seed, "field_of_study_aligned_idx_v1", aligned.len() as u64) as usize;
            aligned[idx].clone()
        } else {
            pick_any_cip(seed)
        }
    } else {
        pick_any_cip(seed)
    };

    let entry = cip_families().iter().find(|f| f.code == cip_code)?;
    Some(FieldOfStudy {
        cip_code: entry.code.clone(),
        title: entry.title.clone(),
    })
}

fn pick_any_cip(seed: u64) -> String {
    let all = cip_families();
    let idx = hash_int(seed, "field_of_study_any_v1", all.len() as u64) as usize;
    all[idx].code.clone()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldOfStudy {
    pub cip_code: String,
    pub title: String,
}

// ---------- Graduation year ----------

/// Year of graduation at this person's highest degree. Returns
/// `birth_year + typical_grad_age(level)`. Future versions could
/// add ±1y noise; for now exact-age keeps tests deterministic and
/// the substrate self-consistent (graduation date ⇒ school year).
pub fn graduation_year_of(person_id: U512) -> u32 {
    let level = education_level_of(person_id);
    let birth = birth_date_of(person_id);
    use chrono::Datelike;
    birth.year() as u32 + level.typical_grad_age()
}

// ---------- Convenience: full education profile ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EducationProfile {
    pub level: EducationLevel,
    pub level_label: String,
    pub field_of_study: Option<FieldOfStudy>,
    pub graduation_year: u32,
    /// The cohort industry this person's education was biased toward
    /// (== `industry_of(person_id)` — included so renderers can
    /// explain the alignment without re-reading the cohort).
    pub cohort_industry: String,
}

pub fn education_profile_of(person_id: U512) -> EducationProfile {
    let level = education_level_of(person_id);
    EducationProfile {
        level,
        level_label: level.label().to_string(),
        field_of_study: field_of_study_of(person_id),
        graduation_year: graduation_year_of(person_id),
        cohort_industry: INDUSTRIES[industry_idx_of(person_id) as usize % INDUSTRIES.len()].to_string(),
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
    fn cip_families_loaded() {
        assert!(cip_families().len() >= 40);
        assert!(cip_families().iter().any(|f| f.code == "11"));
    }

    #[test]
    fn naics_to_cip_loaded_for_all_20_sectors() {
        assert_eq!(naics_to_cip().mapping.len(), 20);
    }

    #[test]
    fn education_level_is_deterministic() {
        let p = pid(8, 0, 1, 7); // Information industry
        assert_eq!(education_level_of(p), education_level_of(p));
    }

    #[test]
    fn education_distribution_is_bachelors_modal() {
        // 5K samples — Bachelors should be the most common (~35% with
        // padding bucket).
        let mut counts = [0u32; 7];
        for raw in 0..5_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let idx = match education_level_of(p) {
                EducationLevel::HighSchool => 0,
                EducationLevel::AssociateDegree => 1,
                EducationLevel::Bachelors => 2,
                EducationLevel::Masters => 3,
                EducationLevel::PhD => 4,
                EducationLevel::MD => 5,
                EducationLevel::JD => 6,
            };
            counts[idx] += 1;
        }
        // Bachelors should win.
        let max_idx = counts.iter().enumerate().max_by_key(|(_, c)| *c).unwrap().0;
        assert_eq!(max_idx, 2, "bachelors not modal: {:?}", counts);
        // PhD/MD/JD combined < 10% — sanity bound.
        let elite = counts[4] + counts[5] + counts[6];
        assert!(elite < 500, "elite degrees too common: {elite}/5000");
    }

    #[test]
    fn high_school_has_no_field_of_study() {
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            if education_level_of(p) == EducationLevel::HighSchool {
                assert!(field_of_study_of(p).is_none());
            } else {
                assert!(field_of_study_of(p).is_some(), "non-HS missing field for mid={mid:#x}");
            }
        }
    }

    #[test]
    fn field_of_study_biases_toward_cohort_industry() {
        // Run 1000 Information-industry persons (idx 8). naics_to_cip[8]
        // = ["11", "09", "50", "27"]. ≥60% of non-HS persons should
        // land in one of those 4 CIP families.
        let mut aligned = 0u32;
        let mut total = 0u32;
        let aligned_set: std::collections::HashSet<_> =
            naics_to_cip().mapping[8].iter().cloned().collect();
        for member in 0..1000u16 {
            let p = pid(8, 0, 1, member);
            if let Some(f) = field_of_study_of(p) {
                total += 1;
                if aligned_set.contains(&f.cip_code) {
                    aligned += 1;
                }
            }
        }
        if total > 0 {
            let frac = aligned as f64 / total as f64;
            assert!(frac >= 0.60, "industry alignment rate too low: {frac}");
        }
    }

    #[test]
    fn graduation_year_consistent_with_level() {
        // Bachelor's grads should graduate around birth_year + 22.
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let level = education_level_of(p);
            let grad = graduation_year_of(p);
            use chrono::Datelike;
            let birth = birth_date_of(p).year() as u32;
            let expected = birth + level.typical_grad_age();
            assert_eq!(grad, expected, "mid={mid:#x} level={level:?} birth={birth} grad={grad}");
        }
    }

    #[test]
    fn education_profile_is_complete() {
        let p = pid(8, 0, 1, 42);
        let prof = education_profile_of(p);
        assert!(!prof.level_label.is_empty());
        assert!(!prof.cohort_industry.is_empty());
        assert!(prof.graduation_year > 1900);
    }
}
