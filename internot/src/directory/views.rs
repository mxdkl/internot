//! The directory's view: `read_person` on the monotone world.
//!
//! Names, households and addresses are not served yet: the world they were
//! built on was replaced (2026-10-03), and they come back as features of the
//! monotone world.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use internot_society::mono::{Mono, Pid};
use internot_society::Sex;
use procedural_core::stream::{from_secs, to_secs};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::universe::Universe;
use crate::views::{DynView, View, ViewError};

const YEAR: f64 = 365.2425 * 86_400.0;

pub fn views() -> Vec<Arc<dyn DynView>> {
    vec![Arc::new(ReadPerson)]
}

// --- records ---------------------------------------------------------------------------

/// Another person, as a reference: id, name, sex and life dates.
#[derive(Debug, Clone, Serialize)]
pub struct PersonRef {
    pub person_id: u64,
    /// Full name at the time asked about (or at death).
    pub name: String,
    pub sex: &'static str,
    pub born: String,
    /// Death date, if before the time asked about.
    pub died: Option<String>,
}

/// A person's union (marriage or cohabitation).
#[derive(Debug, Clone, Serialize)]
pub struct UnionRecord {
    pub partner: PersonRef,
    pub started: String,
    /// When it ended, if before the time asked about.
    pub ended: Option<String>,
    /// separation, partner's death or own death.
    pub how_ended: Option<&'static str>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadPersonParams {
    /// The person's id.
    pub person_id: u64,
    /// The time to read at (ISO 8601 date or date-time, UTC); default: now.
    #[serde(default)]
    pub at: Option<String>,
}

/// Where a person lives at a time.
#[derive(Debug, Clone, Serialize)]
pub struct HomeRecord {
    /// Mail address: "1204 Oak St, Springfield, IL 62704".
    pub address: String,
    pub city: String,
    pub state: &'static str,
    pub zip: Option<String>,
    pub county: String,
    /// When this household moved in.
    pub since: String,
}

/// A stretch of schooling at one institution.
#[derive(Debug, Clone, Serialize)]
pub struct SchoolRecord {
    pub institution: String,
    /// public school, private school, two-year college, public university, …
    pub kind: &'static str,
    pub city: String,
    pub state: &'static str,
    pub from: String,
    pub to: String,
    /// Grades attended, or the degree earned (or "left without a degree").
    pub detail: String,
}

/// A person's education at a time.
#[derive(Debug, Clone, Serialize)]
pub struct EducationRecord {
    /// The highest level completed by then.
    pub completed: Option<&'static str>,
    /// What they were enrolled in then, if anything.
    pub enrolled: Option<String>,
    /// Their schooling up to then.
    pub history: Vec<SchoolRecord>,
}

/// A job.
#[derive(Debug, Clone, Serialize)]
pub struct JobRecord {
    pub title: String,
    /// The occupation, as the Census names it.
    pub occupation: String,
    pub employer: String,
    pub industry: String,
    /// What the employer does, for businesses ("Pizza Parlor").
    pub line_of_business: Option<String>,
    pub city: String,
    pub state: &'static str,
    pub from: String,
    /// When it ended, if before the time asked about.
    pub to: Option<String>,
    /// A student's part-time job.
    pub part_time: bool,
    /// Annual pay in dollars of the time (at the time asked about for a
    /// current job, else at its end).
    pub annual_pay: u64,
}

/// A person's work at a time.
#[derive(Debug, Clone, Serialize)]
pub struct WorkRecord {
    /// employed, unemployed, in school, keeping house, disabled, between
    /// jobs, retired; none before 16 or after death.
    pub status: Option<&'static str>,
    /// Since when that status has held.
    pub since: Option<String>,
    pub job: Option<JobRecord>,
    /// Earlier jobs, most recent first.
    pub history: Vec<JobRecord>,
}

/// A person at a time.
#[derive(Debug, Clone, Serialize)]
pub struct PersonRecord {
    pub person_id: u64,
    pub as_of: String,
    /// alive, deceased or not yet born.
    pub status: &'static str,
    /// Full name (first, middle, surname) at the time asked about, or at
    /// death.
    pub name: String,
    /// Surname at birth, if different from the current one.
    pub birth_surname: Option<String>,
    pub sex: &'static str,
    /// Heritage (race and ethnicity) group, as the pack names it.
    pub heritage: Option<&'static str>,
    pub born: String,
    pub died: Option<String>,
    /// Age at the time asked about (or at death).
    pub age: Option<u32>,
    /// The partner at the time asked about.
    pub partner: Option<PersonRef>,
    /// Unions begun by the time asked about.
    pub unions: Vec<UnionRecord>,
    pub parents: Vec<PersonRef>,
    /// Children born by the time asked about.
    pub children: Vec<PersonRef>,
    /// Siblings born by the time asked about.
    pub siblings: Vec<PersonRef>,
    /// Where they live at the time asked about (alive only).
    pub home: Option<HomeRecord>,
    /// The others living in their household then.
    pub household: Vec<PersonRef>,
    pub education: EducationRecord,
    pub work: WorkRecord,
}

// --- read_person ----------------------------------------------------------------------

pub struct ReadPerson;

impl View for ReadPerson {
    type Params = ReadPersonParams;
    type Output = PersonRecord;
    const NAME: &'static str = "read_person";
    const DESCRIPTION: &'static str = "Read a person at a time (`at`, ISO 8601; default now): name (with the birth surname if it changed), sex, heritage (race and ethnicity) group, home (mail address, county, since when) and household members, education (highest level, current enrollment, schools and colleges attended), work (status, current job with title, employer, industry and pay, and earlier jobs), birth and death dates, age, partner, their union (start, end, how it ended), parents, children and siblings (ids). Follow the ids to read relatives.";

    fn execute(&self, ctx: &Universe, p: ReadPersonParams) -> Result<PersonRecord, ViewError> {
        let w = &ctx.society.world;
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
        let union = w.spouse(x).filter(|&(_, start)| start <= t).map(|(q, start)| {
            let (end, how) = w.union_end(x).expect("a union has an end");
            let ended = (end <= t).then_some(end);
            let how_ended = ended.map(|_| match how {
                0 => "separation",
                _ if death == end => "own death",
                _ => "partner's death",
            });
            (q, start, ended, how_ended)
        });
        let born_by = |c: &Pid| w.birth(*c) <= t;
        Ok(PersonRecord {
            person_id: w.id(x),
            as_of: date_time(t),
            status,
            name: w.full_name(x, name_time(w, x, t)),
            birth_surname: {
                let (b, now) = (w.birth_surname(x), w.surname(x, name_time(w, x, t)));
                (b != now).then(|| w.surname_text(b))
            },
            sex: sex(w, x),
            heritage: w.heritage_name(x),
            born: date(birth),
            died: (death <= t).then(|| date(death)),
            age: (t >= birth).then(|| age(w, x, t.min(death))),
            partner: union
                .filter(|u| status == "alive" && u.2.is_none())
                .map(|u| person_ref(w, u.0, t)),
            unions: union
                .into_iter()
                .map(|(q, start, ended, how_ended)| UnionRecord {
                    partner: person_ref(w, q, t),
                    started: date(start),
                    ended: ended.map(date),
                    how_ended,
                })
                .collect(),
            parents: [w.mother(x), w.father(x)]
                .into_iter()
                .flatten()
                .map(|q| person_ref(w, q, t))
                .collect(),
            children: w.children(x).iter().filter(|c| born_by(c)).map(|&c| person_ref(w, c, t)).collect(),
            siblings: w.siblings(x).iter().filter(|c| born_by(c)).map(|&c| person_ref(w, c, t)).collect(),
            home: (status == "alive").then(|| w.home(x, t)).flatten().map(|h| {
                let (county, _) = w.places().county(h.pos[internot_society::mono::COUNTY]);
                HomeRecord {
                    address: h.line(),
                    city: h.city.clone(),
                    state: h.state,
                    zip: h.zip.map(|z| format!("{z:05}")),
                    county: county.to_string(),
                    since: date(h.dwelling.since),
                }
            }),
            household: match (status == "alive").then(|| w.household(x, t)).flatten() {
                Some(hh) => w.members(hh, t).iter().filter(|&&q| q != x).map(|&q| person_ref(w, q, t)).collect(),
                None => Vec::new(),
            },
            education: education(w, x, t),
            work: work(w, x, t),
        })
    }
}

// --- helpers --------------------------------------------------------------------------

fn person(w: &Mono, id: u64) -> Result<Pid, ViewError> {
    let x = w.pid(id);
    if w.contains(x) {
        Ok(x)
    } else {
        Err(ViewError::NotFound(format!("no person {id}")))
    }
}

fn person_ref(w: &Mono, x: Pid, t: i64) -> PersonRef {
    let death = w.death(x);
    PersonRef {
        person_id: w.id(x),
        name: w.full_name(x, name_time(w, x, t)),
        sex: sex(w, x),
        born: date(w.birth(x)),
        died: (death <= t).then(|| date(death)),
    }
}

/// The time asked about, as seconds (the world's clock).
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

fn sex(w: &Mono, x: Pid) -> &'static str {
    match w.sex(x) {
        Sex::Female => "female",
        Sex::Male => "male",
    }
}

/// A person's education at `t`.
fn education(w: &Mono, x: Pid, t: i64) -> EducationRecord {
    use internot_society::mono::{EducationLevel, Schooling};
    let history: Vec<SchoolRecord> = w
        .education_history(x, t)
        .into_iter()
        .map(|st| {
            let info = w.institution(st.institution);
            let detail = match (st.grades, st.degree) {
                (Some((0, 0)), _) => "kindergarten".to_string(),
                (Some((a, b)), _) if a == b => format!("grade {a}"),
                (Some((0, b)), _) => format!("kindergarten to grade {b}"),
                (Some((a, b)), _) => format!("grades {a}–{b}"),
                (None, Some(d)) => d.name().to_string(),
                (None, None) if st.to < t => "left without a degree".to_string(),
                (None, None) => "enrolled".to_string(),
            };
            SchoolRecord { institution: info.name, kind: info.kind, city: info.city, state: info.state, from: date(st.from), to: date(st.to), detail }
        })
        .collect();
    let enrolled = match w.schooling_at(x, t) {
        Schooling::Kindergarten => Some("kindergarten".to_string()),
        Schooling::School { grade } => Some(format!("grade {grade}")),
        Schooling::College { year, toward } => Some(format!(
            "college, year {year}{}",
            match toward {
                EducationLevel::Associate => " (associate degree)",
                EducationLevel::Bachelor => " (bachelor's degree)",
                _ => "",
            }
        )),
        Schooling::Graduate { year, toward } => Some(format!("graduate school, year {year} ({})", toward.name())),
        Schooling::NotYet | Schooling::Out => None,
    };
    let enrolled = enrolled.map(|e| match history.last() {
        Some(h) if h.to == date(t) || h.detail == "enrolled" || e.starts_with("grade") || e == "kindergarten" => format!("{e} at {}", h.institution),
        _ => e,
    });
    EducationRecord { completed: w.education_at(x, t).map(|l| l.name()), enrolled, history }
}

/// A person's work at `t`: the spell in force and the jobs before it.
fn work(w: &Mono, x: Pid, t: i64) -> WorkRecord {
    use internot_society::mono::{Inactive, WorkSpell};
    let alive = t >= w.birth(x) && t < w.death(x);
    let career = if alive { w.career(x) } else { w.career(x).into_iter().filter(|s| s.span().0 < t).collect() };
    let job = |j: &internot_society::mono::Job| {
        let e = w.employer_info(w.employer(x, j));
        let at = if j.end > t { t } else { j.end - 1 };
        let year = from_secs(at).format("%Y").to_string().parse().unwrap_or(2023);
        JobRecord {
            title: w.job_title(j).to_string(),
            occupation: w.occupation_info(j.occupation).0.to_string(),
            employer: e.name,
            industry: e.industry,
            line_of_business: e.line,
            city: e.city,
            state: e.state,
            from: date(j.start),
            to: (j.end <= t).then(|| date(j.end)),
            part_time: j.part_time,
            annual_pay: {
                // To the nearest $100, or $10 below $1,000.
                let pay = w.nominal(w.pay_at(j, at), year);
                let step = if pay < 1000.0 { 10.0 } else { 100.0 };
                ((pay / step).round() * step).max(10.0) as u64
            },
        }
    };
    let now = career.iter().find(|s| s.span().0 <= t && t < s.span().1).filter(|_| alive);
    let status = now.map(|s| match s {
        WorkSpell::Employed(_) => "employed",
        WorkSpell::Unemployed { .. } => "unemployed",
        WorkSpell::OutOfLaborForce { why: Inactive::School, .. } => "in school",
        WorkSpell::OutOfLaborForce { why: Inactive::Family, .. } => "keeping house",
        WorkSpell::OutOfLaborForce { why: Inactive::Disability, .. } => "disabled",
        WorkSpell::OutOfLaborForce { why: Inactive::Other, .. } => "between jobs",
        WorkSpell::Retired { .. } => "retired",
    });
    let current = match now {
        Some(WorkSpell::Employed(j)) => Some(j.ordinal),
        _ => None,
    };
    WorkRecord {
        status,
        since: now.map(|s| date(s.span().0)),
        job: match now {
            Some(WorkSpell::Employed(j)) => Some(job(j)),
            _ => None,
        },
        history: career
            .iter()
            .rev()
            .filter_map(|s| match s {
                WorkSpell::Employed(j) if j.start < t && Some(j.ordinal) != current => Some(job(j)),
                _ => None,
            })
            .collect(),
    }
}

/// The time a name is read at: `t`, or the last moment of life for the
/// dead.
fn name_time(w: &Mono, x: Pid, t: i64) -> i64 {
    t.min(w.death(x) - 1)
}

fn age(w: &Mono, x: Pid, t: i64) -> u32 {
    ((t - w.birth(x)).max(0) as f64 / YEAR) as u32
}
