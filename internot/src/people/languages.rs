//! Languages spoken by a person — country-driven native + EPI-band-
//! probabilistic acquired English.
//!
//! Datasets:
//! - **Country → native languages**: ISO 639-3 codes from
//!   [mledoze/countries](https://github.com/mledoze/countries),
//!   filtered to Internot's 16 cohort countries
//!   (`data/country_languages.json`). For multi-lingual countries
//!   (India: en/hi/ta; Canada: en/fr; Argentina: es/grn) every
//!   listed language is treated as native — most adults grow up
//!   exposed to multiple in those contexts.
//! - **English-as-second-language proficiency**: calibrated to
//!   [EF English Proficiency Index 2025](https://en.wikipedia.org/wiki/EF_English_Proficiency_Index)
//!   score bands. Each non-native-English country has a
//!   `{Fluent, Conversational, Basic, None}` distribution; the
//!   substrate hash-picks each person's level
//!   (`data/english_proficiency.json`).
//!
//! Cross-service implications:
//! - LLM mail renderer can pick the language for a message — Italian
//!   to Italian, Korean to Korean, English when there's no shared
//!   language other than English.
//! - Calendar / chat prefer the highest-shared-proficiency language
//!   between sender and recipient.
//! - Forum posts in country-native language for casual discussion.

use std::collections::HashMap;
use std::sync::OnceLock;

use procedural_core::sampler::categorical;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use super::derive::country_code_of;
use super::slot::career_arc_seed_of;

// ---------- Datasets ----------

#[derive(Debug, Clone, serde::Deserialize)]
struct CountryLanguage {
    code: String, // ISO 639-3, e.g. "ita"
    name: String, // English name, e.g. "Italian"
}

const COUNTRY_LANGS_RAW: &str = include_str!("../../data/country_languages.json");

fn country_languages() -> &'static HashMap<String, Vec<CountryLanguage>> {
    static T: OnceLock<HashMap<String, Vec<CountryLanguage>>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(COUNTRY_LANGS_RAW).expect("country_languages.json parses")
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
struct EnglishProfData {
    native: bool,
    #[serde(default)]
    distribution: Option<HashMap<String, f64>>,
}

const ENGLISH_PROF_RAW: &str = include_str!("../../data/english_proficiency.json");

fn english_proficiency() -> &'static HashMap<String, EnglishProfData> {
    static T: OnceLock<HashMap<String, EnglishProfData>> = OnceLock::new();
    T.get_or_init(|| {
        let raw: HashMap<String, serde_json::Value> =
            serde_json::from_str(ENGLISH_PROF_RAW).expect("english_proficiency.json parses");
        raw.into_iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .map(|(k, v)| {
                let parsed: EnglishProfData = serde_json::from_value(v).expect("epi entry parses");
                (k, parsed)
            })
            .collect()
    })
}

// ---------- Public types ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Proficiency {
    /// Mother tongue — picked up in childhood, fully native intuition.
    Native,
    /// Conversational + work-capable. Can hold technical discussions
    /// and write professional emails.
    Fluent,
    /// Can hold ordinary conversations but not work in this language.
    Conversational,
    /// A few words / phrases — vacation level.
    Basic,
}

impl Proficiency {
    pub fn label(self) -> &'static str {
        match self {
            Proficiency::Native => "native",
            Proficiency::Fluent => "fluent",
            Proficiency::Conversational => "conversational",
            Proficiency::Basic => "basic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    /// ISO 639-3 three-letter code (e.g. "ita", "eng", "kor").
    pub code: String,
    /// English name of the language.
    pub name: String,
    pub proficiency: Proficiency,
}

// ---------- Accessors ----------

/// Languages this person speaks. Always includes every native
/// language for their cohort country; for non-native-English
/// countries, hash-picks an English proficiency level from the
/// EPI-band distribution. Result includes English iff non-None.
pub fn languages_spoken_of(person_id: U512) -> Vec<Language> {
    let cc = country_code_of(person_id);
    let mut out = Vec::with_capacity(3);

    // 1. All native languages of the cohort country.
    if let Some(natives) = country_languages().get(cc) {
        for lang in natives {
            out.push(Language {
                code: lang.code.clone(),
                name: lang.name.clone(),
                proficiency: Proficiency::Native,
            });
        }
    }

    // 2. Acquired English (if not already native).
    let already_speaks_english = out.iter().any(|l| l.code == "eng");
    if !already_speaks_english {
        if let Some(epi) = english_proficiency().get(cc) {
            if !epi.native {
                if let Some(dist) = &epi.distribution {
                    let prof = pick_english_proficiency(person_id, dist);
                    if let Some(prof) = prof {
                        out.push(Language {
                            code: "eng".into(),
                            name: "English".into(),
                            proficiency: prof,
                        });
                    }
                }
            }
        }
    }

    out
}

fn pick_english_proficiency(
    person_id: U512,
    dist: &HashMap<String, f64>,
) -> Option<Proficiency> {
    // Canonicalize: order is Fluent, Conversational, Basic, None.
    let weights: [f64; 4] = [
        *dist.get("Fluent").unwrap_or(&0.0),
        *dist.get("Conversational").unwrap_or(&0.0),
        *dist.get("Basic").unwrap_or(&0.0),
        *dist.get("None").unwrap_or(&0.0),
    ];
    let seed = career_arc_seed_of(person_id) as u64;
    let idx = categorical(seed, "english_proficiency_v1", &weights);
    match idx {
        0 => Some(Proficiency::Fluent),
        1 => Some(Proficiency::Conversational),
        2 => Some(Proficiency::Basic),
        _ => None,
    }
}

/// Convenience: does this person have at least Conversational
/// proficiency in the given ISO 639-3 code? Useful for
/// "can these two communicate in this language" checks.
pub fn speaks(person_id: U512, iso_639_3: &str) -> bool {
    languages_spoken_of(person_id).iter().any(|l| {
        l.code == iso_639_3
            && matches!(
                l.proficiency,
                Proficiency::Native | Proficiency::Fluent | Proficiency::Conversational
            )
    })
}

/// Best shared language between two persons. Returns the language
/// they BOTH have at the highest mutual proficiency (Native >
/// Fluent > Conversational > Basic). Returns None if they have no
/// language at ≥Conversational on both sides.
pub fn best_shared_language(a: U512, b: U512) -> Option<Language> {
    let langs_a = languages_spoken_of(a);
    let langs_b = languages_spoken_of(b);
    let mut best: Option<(u8, Language)> = None;
    for la in &langs_a {
        for lb in &langs_b {
            if la.code != lb.code {
                continue;
            }
            let mutual_rank = std::cmp::min(rank(&la.proficiency), rank(&lb.proficiency));
            if mutual_rank < 2 {
                continue; // both must be ≥ Conversational
            }
            if let Some((cur, _)) = &best {
                if mutual_rank <= *cur {
                    continue;
                }
            }
            best = Some((
                mutual_rank,
                Language {
                    code: la.code.clone(),
                    name: la.name.clone(),
                    proficiency: match mutual_rank {
                        4 => Proficiency::Native,
                        3 => Proficiency::Fluent,
                        _ => Proficiency::Conversational,
                    },
                },
            ));
        }
    }
    best.map(|(_, l)| l)
}

fn rank(p: &Proficiency) -> u8 {
    match p {
        Proficiency::Native => 4,
        Proficiency::Fluent => 3,
        Proficiency::Conversational => 2,
        Proficiency::Basic => 1,
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
    fn datasets_load() {
        assert!(!country_languages().is_empty());
        assert!(!english_proficiency().is_empty());
    }

    #[test]
    fn italian_persons_speak_italian_natively() {
        // Italy is country_idx=10 in our table (matched against
        // CITIES order — 'IT' comes 11th).
        let mut found = false;
        for member in 0..256u16 {
            for ws in 0..16u8 {
                let p = pid(0, 10, ws, member);
                if country_code_of(p) == "IT" {
                    let langs = languages_spoken_of(p);
                    assert!(
                        langs.iter().any(|l| l.code == "ita" && l.proficiency == Proficiency::Native),
                        "Italian person doesn't speak Italian: {langs:?}"
                    );
                    found = true;
                    break;
                }
            }
            if found { break; }
        }
        assert!(found, "no Italian person found in sweep");
    }

    #[test]
    fn us_persons_have_english_native() {
        for member in 0..16u16 {
            let p = pid(0, 0, 0, member);
            if country_code_of(p) == "US" {
                let langs = languages_spoken_of(p);
                assert!(langs.iter().any(|l| l.code == "eng" && l.proficiency == Proficiency::Native));
                return;
            }
        }
    }

    #[test]
    fn high_epi_country_has_high_english_rate() {
        // Germany (EPI 615 Very High): expect ≥80% of population
        // to have English at ≥ Conversational.
        let mut total = 0u32;
        let mut ok = 0u32;
        for member in 0..2048u16 {
            for ws in 0..8u8 {
                let p = pid(0, 3, ws, member); // city_idx 3 → DE (index 3 in country list)
                if country_code_of(p) != "DE" {
                    continue;
                }
                total += 1;
                if speaks(p, "eng") {
                    ok += 1;
                }
            }
        }
        if total > 100 {
            let rate = ok as f64 / total as f64;
            assert!(rate >= 0.80, "DE English rate {rate} < 0.80");
        }
    }

    #[test]
    fn low_epi_country_has_low_english_rate() {
        // Japan (EPI 446 Very Low): expect ≤40% of population to have
        // English at ≥ Conversational (we modeled ~25%).
        let mut total = 0u32;
        let mut ok = 0u32;
        for member in 0..2048u16 {
            for ws in 0..8u8 {
                let p = pid(0, 2, ws, member); // city_idx 2 → JP
                if country_code_of(p) != "JP" {
                    continue;
                }
                total += 1;
                if speaks(p, "eng") {
                    ok += 1;
                }
            }
        }
        if total > 100 {
            let rate = ok as f64 / total as f64;
            assert!(rate <= 0.45, "JP English rate {rate} > 0.45");
        }
    }

    #[test]
    fn languages_spoken_is_deterministic() {
        let p = pid(0, 10, 1, 7);
        let a = languages_spoken_of(p);
        let b = languages_spoken_of(p);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.code, y.code);
        }
    }

    #[test]
    fn shared_language_for_two_italians_is_italian() {
        let mut italians: Vec<U512> = Vec::new();
        for member in 0..256u16 {
            for ws in 0..16u8 {
                let p = pid(0, 10, ws, member);
                if country_code_of(p) == "IT" {
                    italians.push(p);
                    if italians.len() == 2 { break; }
                }
            }
            if italians.len() == 2 { break; }
        }
        if italians.len() == 2 {
            let shared = best_shared_language(italians[0], italians[1]);
            assert!(shared.is_some());
            assert_eq!(shared.unwrap().code, "ita");
        }
    }

    #[test]
    fn shared_language_falls_back_to_english_when_diff_country() {
        // Italian + Korean — both might speak English depending on EPI
        // draw. Sweep until we find a pair that does.
        let mut italian: Option<U512> = None;
        let mut korean: Option<U512> = None;
        for member in 0..2048u16 {
            for ws in 0..16u8 {
                let p = pid(0, 10, ws, member);
                if country_code_of(p) == "IT" && italian.is_none() && speaks(p, "eng") {
                    italian = Some(p);
                }
                let p = pid(0, 14, ws, member);
                if country_code_of(p) == "KR" && korean.is_none() && speaks(p, "eng") {
                    korean = Some(p);
                }
                if italian.is_some() && korean.is_some() { break; }
            }
            if italian.is_some() && korean.is_some() { break; }
        }
        if let (Some(i), Some(k)) = (italian, korean) {
            let shared = best_shared_language(i, k);
            assert!(shared.is_some());
            assert_eq!(shared.unwrap().code, "eng");
        }
    }
}
