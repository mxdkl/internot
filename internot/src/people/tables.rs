//! Hand-curated reference tables: country names + ISO codes + cities
//! per country + industry list. Small and fixed; sourced from
//! geonamescache for the cities and ISO 3166-1 for the codes.
//!
//! 16 countries × 16 cities each. Country indexing must agree across
//! `COUNTRIES`, `COUNTRY_CODES`, and `CITIES` (same row order).
//!
//! The 16 codes are the locales the names dataset covers densely
//! (US, GB, JP, DE, FR, BR, IN, CN, MX, ES, IT, CA, AR, NG, KR, EG).

pub const COUNTRIES: &[&str] = &[
    "United States", "United Kingdom", "Japan", "Germany",
    "France", "Brazil", "India", "China",
    "Mexico", "Spain", "Italy", "Canada",
    "Argentina", "Nigeria", "South Korea", "Egypt",
];

pub const COUNTRY_CODES: &[&str] = &[
    "US", "GB", "JP", "DE",
    "FR", "BR", "IN", "CN",
    "MX", "ES", "IT", "CA",
    "AR", "NG", "KR", "EG",
];

/// 20 NAICS 2012 sector titles, in canonical sector-code order.
/// Sourced from `internot/data/naics_sectors.json` (distilled from
/// codeforamerica/naics-api by `build_naics.py`). Loaded once via
/// `OnceLock` at first access.
///
/// Why all 20 instead of a hand-picked 16: realism — NAICS is the
/// US Census Bureau's canonical classification, used by every
/// federal employment statistic. Cherry-picking which sectors to
/// drop reintroduces the hand-curation tech debt we're trying to
/// avoid.
///
/// Bit-layout impact: industry_idx is a 6-bit field (0..63). With 20
/// sectors, persons with industry_idx in 20..63 modulo-wrap to
/// sectors 0..3 — meaning some NAICS sectors are over-represented
/// at the bit level but the surface answer is still drawn from the
/// real taxonomy. The bit-pattern pushdown for `workplace_members_of`
/// is unaffected since it pins exact bit-distinct industry_idx
/// values.
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
struct NaicsSector {
    title: String,
    #[serde(default)]
    #[allow(dead_code)] // exposed via INDUSTRIES_RICH below
    sector_code: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
}

const NAICS_RAW: &str = include_str!("../../data/naics_sectors.json");

fn naics_sectors() -> &'static Vec<NaicsSector> {
    static T: OnceLock<Vec<NaicsSector>> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(NAICS_RAW).expect("naics_sectors.json parses"))
}

fn industry_titles() -> &'static Vec<&'static str> {
    static T: OnceLock<Vec<&'static str>> = OnceLock::new();
    T.get_or_init(|| {
        naics_sectors()
            .iter()
            .map(|s| Box::leak(s.title.clone().into_boxed_str()) as &'static str)
            .collect()
    })
}

/// Backward-compatible accessor matching the prior `&[&str]` API.
/// Callers index into this; `industry_of(id)` does so via the
/// existing modulo pattern. Length is 20 (NAICS sectors).
pub struct Industries;

impl Industries {
    pub fn len(&self) -> usize {
        industry_titles().len()
    }

    pub fn is_empty(&self) -> bool {
        industry_titles().is_empty()
    }
}

impl std::ops::Index<usize> for Industries {
    type Output = &'static str;
    fn index(&self, idx: usize) -> &Self::Output {
        &industry_titles()[idx]
    }
}

pub const INDUSTRIES: Industries = Industries;

/// 16 cities per country, indexed by `[country_idx][city_idx]`.
/// Both inner and outer dimensions match the 6-bit fields, but only
/// the first 16 city slots per country are used; `city_idx >= 16`
/// wraps modulo 16.
pub const CITIES: &[[&str; 16]; 16] = &[
    // US
    ["New York", "Los Angeles", "Chicago", "Houston", "Phoenix", "Philadelphia", "San Antonio", "San Diego", "Dallas", "San Jose", "Austin", "Seattle", "Boston", "Denver", "Portland", "Miami"],
    // GB
    ["London", "Birmingham", "Manchester", "Glasgow", "Liverpool", "Leeds", "Sheffield", "Edinburgh", "Bristol", "Cardiff", "Belfast", "Newcastle", "Nottingham", "Brighton", "Leicester", "Aberdeen"],
    // JP
    ["Tokyo", "Yokohama", "Osaka", "Nagoya", "Sapporo", "Fukuoka", "Kobe", "Kyoto", "Kawasaki", "Saitama", "Hiroshima", "Sendai", "Kitakyushu", "Chiba", "Sakai", "Niigata"],
    // DE
    ["Berlin", "Hamburg", "Munich", "Cologne", "Frankfurt", "Stuttgart", "Düsseldorf", "Leipzig", "Dortmund", "Essen", "Bremen", "Dresden", "Hannover", "Nuremberg", "Duisburg", "Bochum"],
    // FR
    ["Paris", "Marseille", "Lyon", "Toulouse", "Nice", "Nantes", "Strasbourg", "Montpellier", "Bordeaux", "Lille", "Rennes", "Reims", "Le Havre", "Saint-Étienne", "Toulon", "Grenoble"],
    // BR
    ["São Paulo", "Rio de Janeiro", "Brasília", "Salvador", "Fortaleza", "Belo Horizonte", "Manaus", "Curitiba", "Recife", "Porto Alegre", "Belém", "Goiânia", "Guarulhos", "Campinas", "São Luís", "Maceió"],
    // IN
    ["Mumbai", "Delhi", "Bangalore", "Hyderabad", "Ahmedabad", "Chennai", "Kolkata", "Pune", "Jaipur", "Surat", "Lucknow", "Kanpur", "Nagpur", "Indore", "Patna", "Bhopal"],
    // CN
    ["Shanghai", "Beijing", "Chongqing", "Guangzhou", "Tianjin", "Shenzhen", "Wuhan", "Chengdu", "Hangzhou", "Xi'an", "Suzhou", "Zhengzhou", "Nanjing", "Qingdao", "Shenyang", "Dalian"],
    // MX
    ["Mexico City", "Guadalajara", "Monterrey", "Puebla", "Toluca", "Tijuana", "León", "Ciudad Juárez", "Zapopan", "Mérida", "San Luis Potosí", "Aguascalientes", "Hermosillo", "Saltillo", "Mexicali", "Culiacán"],
    // ES
    ["Madrid", "Barcelona", "Valencia", "Seville", "Zaragoza", "Málaga", "Murcia", "Palma", "Las Palmas", "Bilbao", "Alicante", "Córdoba", "Valladolid", "Vigo", "Gijón", "L'Hospitalet"],
    // IT
    ["Rome", "Milan", "Naples", "Turin", "Palermo", "Genoa", "Bologna", "Florence", "Bari", "Catania", "Venice", "Verona", "Messina", "Padua", "Trieste", "Brescia"],
    // CA
    ["Toronto", "Montreal", "Calgary", "Ottawa", "Edmonton", "Mississauga", "Winnipeg", "Vancouver", "Brampton", "Hamilton", "Quebec City", "Surrey", "Laval", "Halifax", "London", "Markham"],
    // AR
    ["Buenos Aires", "Córdoba", "Rosario", "Mendoza", "La Plata", "San Miguel de Tucumán", "Mar del Plata", "Salta", "Santa Fe", "San Juan", "Resistencia", "Neuquén", "Bahía Blanca", "Posadas", "Paraná", "Corrientes"],
    // NG
    ["Lagos", "Kano", "Ibadan", "Benin City", "Port Harcourt", "Jos", "Ilorin", "Abuja", "Kaduna", "Maiduguri", "Zaria", "Aba", "Onitsha", "Warri", "Sokoto", "Ogbomoso"],
    // KR
    ["Seoul", "Busan", "Incheon", "Daegu", "Daejeon", "Gwangju", "Suwon", "Ulsan", "Changwon", "Goyang", "Yongin", "Seongnam", "Cheongju", "Bucheon", "Hwaseong", "Ansan"],
    // EG
    ["Cairo", "Alexandria", "Giza", "Shubra El Kheima", "Port Said", "Suez", "Luxor", "Mansoura", "El Mahalla El Kubra", "Tanta", "Asyut", "Ismailia", "Faiyum", "Zagazig", "Aswan", "Damietta"],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_match_each_other() {
        assert_eq!(COUNTRIES.len(), COUNTRY_CODES.len());
        assert_eq!(COUNTRIES.len(), CITIES.len());
        // INDUSTRIES is now 20 NAICS sectors (loaded from JSON, no
        // longer hand-curated to 16). The 6-bit industry_idx field
        // can hold values 0..63; persons with idx ≥ 20 modulo-wrap.
        assert_eq!(INDUSTRIES.len(), 20);
    }

    #[test]
    fn industries_are_naics_sectors() {
        // Spot-check that the swap landed: real NAICS titles, not
        // the old hand-curated single-word labels.
        let titles: Vec<&str> = (0..INDUSTRIES.len()).map(|i| INDUSTRIES[i]).collect();
        assert!(titles.contains(&"Manufacturing"));
        assert!(titles.contains(&"Construction"));
        assert!(titles.contains(&"Public Administration"));
        assert!(titles.contains(&"Finance and Insurance"));
        // Sanity: every title is non-empty and reasonably long
        // (NAICS titles are 1-9 words; "Information" is the
        // shortest at 11 chars).
        for t in titles {
            assert!(t.len() >= 5, "industry title too short: {:?}", t);
        }
    }

    #[test]
    fn city_rows_each_have_16_entries() {
        for (i, row) in CITIES.iter().enumerate() {
            assert_eq!(row.len(), 16, "country row {} has {} cities", i, row.len());
        }
    }
}
