//! The directory's views: `read_person` and `read_household`.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use internot_society::residence::{AREA, COUNTY, TRACT, ZONE};
use procedural_core::stream::{from_secs, to_secs};
use internot_society::{Household, PersonId, Sex, World};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::society::Society;
use crate::universe::Universe;
use crate::views::{DynView, View, ViewError};

const YEAR: f64 = 365.2425 * 86_400.0;

pub fn views() -> Vec<Arc<dyn DynView>> {
    vec![Arc::new(ReadPerson), Arc::new(ReadHousehold)]
}

// --- shared records -------------------------------------------------------------------

/// Another person, as a reference: id, name and life dates.
#[derive(Debug, Clone, Serialize)]
pub struct PersonRef {
    pub person_id: u32,
    pub name: String,
    pub sex: &'static str,
    pub born: String,
    /// Death date, if before the time asked about.
    pub died: Option<String>,
}

/// Where a household lives: a 2020 census tract and the places above it.
#[derive(Debug, Clone, Serialize)]
pub struct Address {
    /// The tract's census GEOID.
    pub tract: u64,
    pub county: String,
    pub county_fips: u32,
    pub commuting_zone: String,
    /// The residence area (whole commuting zones of a state).
    pub area: String,
    /// The tract's population centre.
    pub latitude: f64,
    pub longitude: f64,
}

/// A household member and how they relate to the person asked about.
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub person: PersonRef,
    pub age: u32,
    /// self, partner, child, parent, sibling, grandchild, grandparent,
    /// relative (other kin), roommate or other.
    pub relation: &'static str,
}

/// A household at a time.
#[derive(Debug, Clone, Serialize)]
pub struct HouseholdRecord {
    /// couple (a union's home), single (one adult's own home) or roommates.
    pub kind: &'static str,
    pub members: Vec<Member>,
    pub address: Address,
}

/// One of a person's unions (marriage or cohabitation).
#[derive(Debug, Clone, Serialize)]
pub struct UnionRecord {
    pub partner: PersonRef,
    pub started: String,
    /// When it ended, if before the time asked about.
    pub ended: Option<String>,
    /// separation, partner's death or own death.
    pub how_ended: Option<&'static str>,
    /// When the couple married, if they did by the time asked about.
    pub married: Option<String>,
}

// --- read_person ----------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadPersonParams {
    /// The person's id.
    pub person_id: u32,
    /// The time to read at (ISO 8601 date or date-time, UTC); default: now.
    #[serde(default)]
    pub at: Option<String>,
}

/// A person at a time.
#[derive(Debug, Clone, Serialize)]
pub struct PersonRecord {
    pub person_id: u32,
    pub as_of: String,
    /// alive, deceased or not yet born.
    pub status: &'static str,
    pub full_name: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub surname: String,
    /// The surname at birth, if different now.
    pub birth_surname: Option<String>,
    pub sex: &'static str,
    pub born: String,
    pub died: Option<String>,
    /// Age at the time asked about (or at death).
    pub age: Option<u32>,
    pub heritage: String,
    /// Arrival date, for people born abroad.
    pub immigrated: Option<String>,
    /// The lineage region of the person's birth cohort (in area mode, the
    /// area they grew up in).
    pub home_region: String,
    pub partner: Option<PersonRef>,
    pub household: Option<HouseholdRecord>,
    pub unions: Vec<UnionRecord>,
    pub parents: Vec<PersonRef>,
    pub children: Vec<PersonRef>,
    pub siblings: Vec<PersonRef>,
}

pub struct ReadPerson;

impl View for ReadPerson {
    type Params = ReadPersonParams;
    type Output = PersonRecord;
    const NAME: &'static str = "read_person";
    const DESCRIPTION: &'static str = "Read a person at a time (`at`, ISO 8601; default now): names (with marriage surname changes), sex, birth and death dates, age, heritage, immigration, partner, every union (start, end, how it ended, marriage date), parents, children and siblings (ids and names), and the household they live in with its members and address (census tract, county, commuting zone, area, coordinates). Follow the ids to read relatives.";

    fn execute(&self, ctx: &Universe, p: ReadPersonParams) -> Result<PersonRecord, ViewError> {
        let s = ctx.society;
        let w = &s.world;
        let x = person(w, p.person_id)?;
        let t = time(ctx, p.at.as_deref())?;
        let (birth, death) = (w.birth(x), w.death(x));
        let status = if t < birth {
            "not yet born"
        } else if t >= death {
            "deceased"
        } else {
            "alive"
        };
        // Names as they were at the time (or at death).
        let tn = name_time(w, x, t);
        let surname = w.surname_text(w.surname(x, tn));
        let birth_surname = w.surname_text(w.birth_surname(x));
        let params = &w.ledger().params;
        let living = status == "alive";
        let unions: Vec<UnionRecord> = w
            .unions(x)
            .into_iter()
            .flatten()
            .filter(|u| u.start <= t)
            .map(|u| {
                let ended = (u.end <= t).then_some(u.end);
                let how_ended = ended.map(|end| {
                    if u.separation == Some(end) {
                        "separation"
                    } else if w.death(x) == end {
                        "own death"
                    } else {
                        "partner's death"
                    }
                });
                UnionRecord {
                    partner: person_ref(w, u.partner, t),
                    started: date(u.start),
                    ended: ended.map(date),
                    how_ended,
                    married: w.marriage_date(x, &u).filter(|&m| m <= t).map(date),
                }
            })
            .collect();
        Ok(PersonRecord {
            person_id: x,
            as_of: date_time(t),
            status,
            full_name: w.full_name(x, tn),
            first_name: w.first_name(x).to_string(),
            middle_name: w.middle_name(x).map(str::to_string),
            birth_surname: (birth_surname != surname).then_some(birth_surname),
            surname,
            sex: sex(w, x),
            born: date(birth),
            died: (death <= t).then(|| date(death)),
            age: (t >= birth).then(|| age(w, x, t.min(death))),
            heritage: params.heritage.groups[w.heritage(x).index()].name.clone(),
            immigrated: w.is_immigrant(x).then(|| w.arrival(x).map(date)).flatten(),
            home_region: params.regions[w.region(x) as usize].name.clone(),
            partner: living
                .then(|| w.partner_at(x, t))
                .flatten()
                .map(|q| person_ref(w, q, t)),
            household: living.then(|| household(s, x, t)).flatten(),
            unions,
            parents: [w.mother(x), w.father(x)]
                .into_iter()
                .flatten()
                .map(|q| person_ref(w, q, t))
                .collect(),
            children: w
                .children(x)
                .iter()
                .filter(|&&c| w.birth(c) <= t)
                .map(|&c| person_ref(w, c, t))
                .collect(),
            siblings: w
                .siblings(x)
                .iter()
                .filter(|&&c| w.birth(c) <= t)
                .map(|&c| person_ref(w, c, t))
                .collect(),
        })
    }
}

// --- read_household -------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadHouseholdParams {
    /// Anyone living in the household.
    pub person_id: u32,
    /// The time to read at (ISO 8601 date or date-time, UTC); default: now.
    #[serde(default)]
    pub at: Option<String>,
}

pub struct ReadHousehold;

impl View for ReadHousehold {
    type Params = ReadHouseholdParams;
    type Output = HouseholdRecord;
    const NAME: &'static str = "read_household";
    const DESCRIPTION: &'static str = "Read the household a person lives in at a time (`at`, ISO 8601; default now): its kind (couple, single or roommates), every member with age and relation to the person, and its address (census tract, county, commuting zone, area, coordinates).";

    fn execute(&self, ctx: &Universe, p: ReadHouseholdParams) -> Result<HouseholdRecord, ViewError> {
        let s = ctx.society;
        let x = person(&s.world, p.person_id)?;
        let t = time(ctx, p.at.as_deref())?;
        household(s, x, t).ok_or_else(|| {
            ViewError::NotFound(format!("person {x} is not alive and in the world at {}", date_time(t)))
        })
    }
}

// --- helpers --------------------------------------------------------------------------

fn person(w: &World, id: u32) -> Result<PersonId, ViewError> {
    if (id as u64) < w.population() {
        Ok(id)
    } else {
        Err(ViewError::NotFound(format!(
            "no person {id} (ids run from 0 to {})",
            w.population() - 1
        )))
    }
}

/// The time asked about, as seconds since 1800 (the world's clock).
fn time(ctx: &Universe, at: Option<&str>) -> Result<i64, ViewError> {
    let Some(at) = at else {
        return Ok(to_secs(ctx.now));
    };
    if let Ok(t) = DateTime::parse_from_rfc3339(at) {
        return Ok(to_secs(t.with_timezone(&Utc)));
    }
    let d = NaiveDate::parse_from_str(at, "%Y-%m-%d").map_err(|_| {
        ViewError::InvalidParams(format!("`at` must be an ISO 8601 date or date-time, got `{at}`"))
    })?;
    let t = d.and_hms_opt(12, 0, 0).expect("noon exists").and_utc();
    Ok(to_secs(t))
}

fn date(s: i64) -> String {
    from_secs(s).format("%Y-%m-%d").to_string()
}

fn date_time(s: i64) -> String {
    from_secs(s).format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn sex(w: &World, x: PersonId) -> &'static str {
    match w.sex(x) {
        Sex::Female => "female",
        Sex::Male => "male",
    }
}

fn age(w: &World, x: PersonId, t: i64) -> u32 {
    ((t - w.birth(x)).max(0) as f64 / YEAR) as u32
}

/// The time to read `x`'s name at: `t`, within their life.
fn name_time(w: &World, x: PersonId, t: i64) -> i64 {
    t.min(w.death(x) - 1).max(w.birth(x))
}

fn person_ref(w: &World, x: PersonId, t: i64) -> PersonRef {
    let death = w.death(x);
    PersonRef {
        person_id: x,
        name: w.full_name(x, name_time(w, x, t)),
        sex: sex(w, x),
        born: date(w.birth(x)),
        died: (death <= t).then(|| date(death)),
    }
}

/// How `m` relates to `x` (both living in one household at `t`).
fn relation(w: &World, x: PersonId, m: PersonId, h: &Household, t: i64) -> &'static str {
    let parents = |p: PersonId| [w.mother(p), w.father(p)];
    if m == x {
        "self"
    } else if w.partner_at(x, t) == Some(m) {
        "partner"
    } else if parents(m).contains(&Some(x)) {
        "child"
    } else if parents(x).contains(&Some(m)) {
        "parent"
    } else if w.siblings(x).contains(&m) {
        "sibling"
    } else if parents(m).into_iter().flatten().any(|p| parents(p).contains(&Some(x))) {
        "grandchild"
    } else if parents(x).into_iter().flatten().any(|p| parents(p).contains(&Some(m))) {
        "grandparent"
    } else if matches!(h, Household::Roommates { .. }) {
        "roommate"
    } else if w.siblings(x).iter().any(|&s| parents(m).contains(&Some(s)))
        || parents(x).into_iter().flatten().any(|p| w.siblings(p).contains(&m))
    {
        "relative"
    } else {
        "other"
    }
}

/// `x`'s household at `t`, with members and address, if `x` lives in the
/// world then.
fn household(s: &Society, x: PersonId, t: i64) -> Option<HouseholdRecord> {
    let w = &s.world;
    let h = w.household(x, t)?;
    let kind = match h {
        Household::Union { .. } => "couple",
        Household::Solo { .. } => "single",
        Household::Roommates { .. } => "roommates",
    };
    let members = w
        .members(h, t)
        .iter()
        .map(|&m| Member {
            person: person_ref(w, m, t),
            age: age(w, m, t),
            relation: relation(w, x, m, &h, t),
        })
        .collect();
    let pos = s.residence.address(w, h, t);
    let places = s.residence.places();
    let (county, fips) = places.county(pos[COUNTY]).unwrap_or(("", 0));
    let at = places.tract_at(pos[TRACT]);
    Some(HouseholdRecord {
        kind,
        members,
        address: Address {
            tract: places.tract_geoid(pos[TRACT]).unwrap_or(0),
            county: county.to_string(),
            county_fips: fips,
            commuting_zone: places.zone_name(pos[ZONE]).unwrap_or("").to_string(),
            area: places.area_name(pos[AREA]).unwrap_or("").to_string(),
            latitude: at.lat,
            longitude: at.lon,
        },
    })
}
