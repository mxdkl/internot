//! Per-country timezone data, distilled from vvo/tzdb (IANA + geonames).
//!
//! Loaded once via `OnceLock` at first access; `country_timezones.json`
//! is `include_str!()`'d at compile time, so deployment ships a single
//! binary with no external file dependency.
//!
//! Lookup is by ISO 3166-1 alpha-2 country code (e.g. "US", "JP").
//! Multi-timezone countries are mapped to their most-populous zone
//! (US → America/New_York, RU → Europe/Moscow, BR → America/Sao_Paulo)
//! per the `MULTI_TZ_OVERRIDES` table in `internot/build_tzdb.py`.
//!
//! Why this isn't per-city: city-level timezones would require joining
//! geonamescache against IANA boundary data — heavier dataset and
//! marginal realism gain since most queries care about working-hours
//! windows that are stable within a country's primary zone.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const RAW: &str = include_str!("../../data/country_timezones.json");

#[derive(Debug, Clone, Deserialize)]
pub struct CountryTimezone {
    pub tz: String,
    pub offset_minutes: i32,
    pub primary_city: String,
}

fn table() -> &'static HashMap<String, CountryTimezone> {
    static T: OnceLock<HashMap<String, CountryTimezone>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(RAW).expect("country_timezones.json parses")
    })
}

/// Lookup by 2-letter country code. Returns `None` for unknown codes
/// (the substrate's `country_code_of` should always produce one of
/// the 16 codes we cover, so misses are programming bugs).
pub fn lookup(country_code: &str) -> Option<&'static CountryTimezone> {
    table().get(country_code)
}

/// IANA timezone string for a country code. Falls back to "UTC" if
/// the code isn't in the dataset.
pub fn tz_of(country_code: &str) -> &'static str {
    lookup(country_code).map(|c| c.tz.as_str()).unwrap_or("UTC")
}

/// Raw UTC offset in minutes for a country code, ignoring DST.
/// Used by working-hours and quiet-hours derivations.
pub fn offset_minutes_of(country_code: &str) -> i32 {
    lookup(country_code).map(|c| c.offset_minutes).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_countries_resolve() {
        assert_eq!(tz_of("US"), "America/New_York");
        assert_eq!(tz_of("JP"), "Asia/Tokyo");
        assert_eq!(tz_of("IT"), "Europe/Rome");
        assert_eq!(tz_of("BR"), "America/Sao_Paulo");
    }

    #[test]
    fn offsets_are_realistic() {
        assert_eq!(offset_minutes_of("JP"), 540); // UTC+9
        assert_eq!(offset_minutes_of("GB"), 0);   // UTC
        assert_eq!(offset_minutes_of("US"), -300); // EST -5
    }

    #[test]
    fn unknown_country_falls_back_to_utc() {
        assert_eq!(tz_of("ZZ"), "UTC");
        assert_eq!(offset_minutes_of("ZZ"), 0);
    }

    #[test]
    fn dataset_covers_the_16_substrate_countries() {
        // The 16 country codes the names dataset + bit layout target.
        for cc in [
            "US", "GB", "JP", "DE", "FR", "BR", "IN", "CN",
            "MX", "ES", "IT", "CA", "AR", "NG", "KR", "EG",
        ] {
            assert!(
                lookup(cc).is_some(),
                "country code {} missing from country_timezones.json",
                cc
            );
        }
    }
}
