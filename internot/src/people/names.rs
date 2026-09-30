//! Locale + gender aware procedural names.
//!
//! Source: a 491M-record real-name dataset, deduplicated and top-N'd
//! per `(country, gender)` for first names and per `country` for last
//! names. Compiled into `data/first_names.json` (1.8MB) and
//! `data/last_names.json` (3.9MB) via `build_names.py`. Both are
//! `include_str!`'d at compile time and parsed once via `OnceLock`.
//!
//! Per learning.md: this replaces the prior 64-entry hand-coded
//! `FIRST_NAMES`/`LAST_NAMES` tables that produced "Alex Smith"
//! everywhere regardless of country.

use std::collections::HashMap;
use std::sync::OnceLock;

const FIRST_JSON: &str = include_str!("../../data/first_names.json");
const LAST_JSON: &str = include_str!("../../data/last_names.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    pub fn as_char(self) -> char {
        match self {
            Gender::Male => 'M',
            Gender::Female => 'F',
        }
    }
}

fn first_table() -> &'static HashMap<String, Vec<String>> {
    static T: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(FIRST_JSON).expect("first_names.json is well-formed")
    })
}

fn last_table() -> &'static HashMap<String, Vec<String>> {
    static T: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(LAST_JSON).expect("last_names.json is well-formed")
    })
}

/// First name keyed on `(country_code, gender, name_idx)`. Returns
/// `None` if the dataset doesn't cover the locale (caller falls back).
pub fn first_name_for(country_code: &str, gender: Gender, name_idx: u64) -> Option<&'static str> {
    let key = format!("{}_{}", country_code, gender.as_char());
    let bucket = first_table().get(&key)?;
    if bucket.is_empty() {
        return None;
    }
    Some(bucket[(name_idx as usize) % bucket.len()].as_str())
}

/// Last name keyed on `(country_code, name_idx)`. Returns `None` if
/// the locale isn't in the dataset.
pub fn last_name_for(country_code: &str, name_idx: u64) -> Option<&'static str> {
    let bucket = last_table().get(country_code)?;
    if bucket.is_empty() {
        return None;
    }
    Some(bucket[(name_idx as usize) % bucket.len()].as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_name_returns_some_for_us_male() {
        let n = first_name_for("US", Gender::Male, 0);
        assert!(n.is_some());
    }

    #[test]
    fn last_name_returns_some_for_us() {
        let n = last_name_for("US", 0);
        assert!(n.is_some());
    }

    #[test]
    fn first_name_is_deterministic() {
        let a = first_name_for("JP", Gender::Female, 12345);
        let b = first_name_for("JP", Gender::Female, 12345);
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_locale_returns_none() {
        assert!(first_name_for("XYZZY", Gender::Male, 0).is_none());
        assert!(last_name_for("XYZZY", 0).is_none());
    }
}
