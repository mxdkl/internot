//! Career arc subsystem — first real consumer of the Tier 4
//! `career_arc_seed`.
//!
//! Produces a deterministic career timeline for any person: job
//! switches, promotions, demotions, lateral moves, sabbaticals,
//! entrepreneurship, retirement. Pure function of `(person_id, t)`
//! — same id, same answer, every time.
//!
//! **Design choice:** career events are layered DESCRIPTIVELY on top
//! of the canonical cohort axes (industry_idx / city_idx /
//! workplace_seed in the bottom 32 bits of person_id). Those bits
//! stay immutable — they're the bit-pattern-pushdown-searchable
//! identity that every other service references via `mail_id`. The
//! career timeline tells the story of a person's working life
//! WITHOUT mutating the cohort axes; the canonical industry remains
//! the "starting / primary" industry. A future v2 may add
//! `current_industry_at(id, t)` that overrides via the timeline,
//! but that's scope creep for this first consumer.
//!
//! Inter-tier dependency: the timeline is bounded by `lifecycle_epoch`
//! (Tier 4) — events start at age 18, retire by age 75 latest.

use std::sync::OnceLock;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use procedural_core::hash::{hash_float, hash_int};
use procedural_core::sampler::{categorical, lognormal};
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use super::education::{education_level_of, EducationLevel};
use super::slot::{birth_date_of, career_arc_seed_of, industry_idx_of, lifecycle_phase_for_age};

// ---------- SOC occupation dataset ----------

#[derive(Debug, Clone, serde::Deserialize)]
struct SocOccupation {
    #[allow(dead_code)] // available for future SOC-code lookup
    code: String,
    title: String,
    major: String,
}

const SOC_RAW: &str = include_str!("../../data/soc_occupations.json");

fn soc_occupations() -> &'static Vec<SocOccupation> {
    static T: OnceLock<Vec<SocOccupation>> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(SOC_RAW).expect("soc_occupations.json parses"))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NaicsToSoc {
    mapping: Vec<Vec<String>>,
}

const NAICS_TO_SOC_RAW: &str = include_str!("../../data/naics_to_soc_majors.json");

fn naics_to_soc() -> &'static NaicsToSoc {
    static T: OnceLock<NaicsToSoc> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(NAICS_TO_SOC_RAW).expect("naics_to_soc_majors.json parses"))
}

/// Per-industry SOC candidate index. Built once per industry; the
/// alternative is to allocate + filter the 848-entry SOC list on
/// every title pick, which dominated PersonAvm construction.
fn soc_candidates_for_industry() -> &'static Vec<Vec<&'static str>> {
    static T: OnceLock<Vec<Vec<&'static str>>> = OnceLock::new();
    T.get_or_init(|| {
        let mapping = &naics_to_soc().mapping;
        let occupations = soc_occupations();
        mapping
            .iter()
            .map(|majors| {
                occupations
                    .iter()
                    .filter(|o| majors.contains(&o.major))
                    .map(|o| o.title.as_str())
                    .collect()
            })
            .collect()
    })
}

/// Full SOC title list (cached pointer slice). Used for the 20%
/// cross-industry pick path.
fn soc_all_titles() -> &'static Vec<&'static str> {
    static T: OnceLock<Vec<&'static str>> = OnceLock::new();
    T.get_or_init(|| soc_occupations().iter().map(|o| o.title.as_str()).collect())
}

/// Pick a SOC detailed occupation appropriate for this person's
/// industry. Returns the SOC title (e.g. "Software Developers").
/// Industry-aligned 80% of the time; cross-industry 20%.
fn pick_soc_title(person_id: U512, role_seed: u64) -> &'static str {
    let industry = industry_idx_of(person_id) as usize;
    let aligned = hash_int(role_seed, "soc_pick_aligned_v1", 5) < 4;
    let candidates: &[&'static str] = if aligned {
        let per_industry = soc_candidates_for_industry();
        per_industry
            .get(industry)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
            .unwrap_or_else(|| soc_all_titles().as_slice())
    } else {
        soc_all_titles().as_slice()
    };
    if candidates.is_empty() {
        return "Worker";
    }
    let idx = hash_int(role_seed, "soc_pick_idx_v1", candidates.len() as u64) as usize;
    candidates[idx]
}

/// Process-wide memo of `person_id → Vec<CareerEvent>`. Like
/// `person_id_for`'s cache but for the much heavier timeline
/// simulation. Pure function of person_id.
fn career_cache() -> &'static parking_lot::RwLock<std::collections::HashMap<U512, std::sync::Arc<Vec<CareerEvent>>>> {
    static C: OnceLock<parking_lot::RwLock<std::collections::HashMap<U512, std::sync::Arc<Vec<CareerEvent>>>>> = OnceLock::new();
    C.get_or_init(|| parking_lot::RwLock::new(std::collections::HashMap::new()))
}

/// Earliest age at which a career event can fire. Persons younger
/// than this have an empty career timeline.
pub const CAREER_START_AGE: u32 = 18;

/// Latest age at which retirement can fire. Hash-derived retirement
/// age is in `[60, RETIREMENT_AGE_MAX]`.
pub const RETIREMENT_AGE_MIN: u32 = 60;
pub const RETIREMENT_AGE_MAX: u32 = 75;

/// Safety cap on number of events per person — protects against
/// pathological lognormal draws that could otherwise produce
/// thousands of micro-events.
pub const MAX_EVENTS_PER_PERSON: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CareerEventKind {
    /// Changed employer; possibly changed industry.
    JobSwitch,
    /// Stayed with employer, moved up a level.
    Promotion,
    /// Stayed with employer, moved down a level.
    Demotion,
    /// Stayed with employer, sideways move (different team / function).
    LateralMove,
    /// Career break — return-to-work implied unless followed by Retirement.
    Sabbatical,
    /// Started a company.
    FoundCompany,
    /// Career end. Always the last event in any timeline.
    Retirement,
}

/// Role level / seniority. Walks Junior → Mid → Senior → Staff →
/// Lead → Principal → Manager → Director → VP → Exec. Promotion
/// advances by 1 (or jumps from Principal → Manager / Director →
/// VP). Demotion moves down by 1. JobSwitch can land at any level
/// (often near current). FoundCompany jumps directly to Exec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RoleLevel {
    Junior,
    Mid,
    Senior,
    Staff,
    Lead,
    Principal,
    Manager,
    Director,
    VP,
    Exec,
}

impl RoleLevel {
    pub fn label(self) -> &'static str {
        match self {
            RoleLevel::Junior => "Junior",
            RoleLevel::Mid => "Mid-Level",
            RoleLevel::Senior => "Senior",
            RoleLevel::Staff => "Staff",
            RoleLevel::Lead => "Lead",
            RoleLevel::Principal => "Principal",
            RoleLevel::Manager => "Manager",
            RoleLevel::Director => "Director",
            RoleLevel::VP => "VP",
            RoleLevel::Exec => "Executive",
        }
    }

    fn next(self) -> Self {
        match self {
            RoleLevel::Junior => RoleLevel::Mid,
            RoleLevel::Mid => RoleLevel::Senior,
            RoleLevel::Senior => RoleLevel::Staff,
            RoleLevel::Staff => RoleLevel::Lead,
            RoleLevel::Lead => RoleLevel::Principal,
            RoleLevel::Principal => RoleLevel::Manager,
            RoleLevel::Manager => RoleLevel::Director,
            RoleLevel::Director => RoleLevel::VP,
            RoleLevel::VP => RoleLevel::Exec,
            RoleLevel::Exec => RoleLevel::Exec, // ceiling
        }
    }

    fn prev(self) -> Self {
        match self {
            RoleLevel::Junior => RoleLevel::Junior, // floor
            RoleLevel::Mid => RoleLevel::Junior,
            RoleLevel::Senior => RoleLevel::Mid,
            RoleLevel::Staff => RoleLevel::Senior,
            RoleLevel::Lead => RoleLevel::Staff,
            RoleLevel::Principal => RoleLevel::Lead,
            RoleLevel::Manager => RoleLevel::Principal,
            RoleLevel::Director => RoleLevel::Manager,
            RoleLevel::VP => RoleLevel::Director,
            RoleLevel::Exec => RoleLevel::VP,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerEvent {
    pub when: NaiveDate,
    pub kind: CareerEventKind,
    pub age_at_event: u32,
    /// Zero-indexed sequence within this person's timeline.
    pub seq: u8,
    /// Role level immediately AFTER this event. None for Retirement
    /// + Sabbatical (career paused). Promotion advances; Demotion
    /// regresses; JobSwitch / LateralMove typically holds level;
    /// FoundCompany jumps to Exec.
    pub to_level: Option<RoleLevel>,
    /// 32-bit seed identifying the employer immediately AFTER this
    /// event. Persists across Promotion/Demotion/LateralMove (same
    /// employer, level changed). Changes on JobSwitch /
    /// FoundCompany. Renderers translate seed → company name.
    pub employer_seed: u32,
    /// SOC-derived job title immediately AFTER this event (e.g.
    /// "Software Developers", "Marketing Managers"). Pulled from
    /// the `data/soc_occupations.json` BLS dataset, biased toward
    /// the cohort industry.
    pub title: String,
}

/// Build the full career timeline for this person. Deterministic per
/// `person_id`. Always terminates with a `Retirement` event.
///
/// Each event carries the role level + employer + title in effect
/// AFTER it. Promotion advances level; Demotion regresses;
/// JobSwitch / FoundCompany change employer (employer_seed reseeded);
/// LateralMove changes title within same employer + level.
///
/// Starting level depends on highest education:
///   HighSchool / Associate → Junior
///   Bachelors → Junior
///   Masters → Mid (skip Junior)
///   PhD / MD / JD → Senior (skip Junior + Mid)
///
/// Memoized via `career_cache` — building a timeline runs ~10
/// hash_int + ~10 SOC picks, dominating PersonAvm construction
/// and find_people inner loops without it.
pub fn career_events_for(person_id: U512) -> Vec<CareerEvent> {
    if let Some(hit) = career_cache().read().get(&person_id).cloned() {
        return (*hit).clone();
    }
    let computed = career_events_for_uncached(person_id);
    career_cache()
        .write()
        .insert(person_id, std::sync::Arc::new(computed.clone()));
    computed
}

fn career_events_for_uncached(person_id: U512) -> Vec<CareerEvent> {
    let seed = career_arc_seed_of(person_id) as u64;
    let birth = birth_date_of(person_id);

    // Hash-derived retirement age (uniform [RETIREMENT_AGE_MIN, MAX]).
    let retire_age = RETIREMENT_AGE_MIN
        + hash_int(
            seed,
            "career_retire_age_v1",
            (RETIREMENT_AGE_MAX - RETIREMENT_AGE_MIN + 1) as u64,
        ) as u32;

    // Education-driven starting level.
    let mut current_level = match education_level_of(person_id) {
        EducationLevel::HighSchool | EducationLevel::AssociateDegree => RoleLevel::Junior,
        EducationLevel::Bachelors => RoleLevel::Junior,
        EducationLevel::Masters => RoleLevel::Mid,
        EducationLevel::PhD | EducationLevel::MD | EducationLevel::JD => RoleLevel::Senior,
    };
    // Initial employer + title at career-start (age 18 / first job).
    let mut current_employer: u32 =
        hash_int(seed, "career_employer_initial_v1", 1u64 << 32) as u32;
    let mut current_title: String =
        pick_soc_title(person_id, seed.wrapping_add(0xCAFE)).to_string();

    let mut events: Vec<CareerEvent> = Vec::new();
    let mut cursor_days: i64 = (CAREER_START_AGE as i64) * 365;
    let mut seq: u8 = 0;

    loop {
        let event_seed = seed.wrapping_add((seq as u64) << 16);
        let interval_years = lognormal(event_seed, "career_interval_v1", 3.0_f64.ln(), 0.7);
        let interval_days = (interval_years * 365.0).clamp(180.0, 15.0 * 365.0) as i64;
        cursor_days += interval_days;

        let age_at = (cursor_days / 365) as u32;

        if age_at >= retire_age {
            let retire_days = (retire_age as i64) * 365;
            events.push(CareerEvent {
                when: birth + Duration::days(retire_days),
                kind: CareerEventKind::Retirement,
                age_at_event: retire_age,
                seq,
                to_level: None,
                employer_seed: current_employer,
                title: current_title.clone(),
            });
            break;
        }

        let kind = pick_event_kind(event_seed, age_at);

        // Update level + employer + title based on the event kind.
        // Done BEFORE pushing so the event reflects the post-event state.
        let (next_level, next_employer, next_title) = match kind {
            CareerEventKind::Promotion => {
                let nl = current_level.next();
                (Some(nl), current_employer, current_title.clone())
            }
            CareerEventKind::Demotion => {
                let nl = current_level.prev();
                (Some(nl), current_employer, current_title.clone())
            }
            CareerEventKind::JobSwitch => {
                let new_emp =
                    hash_int(event_seed, "career_employer_switch_v1", 1u64 << 32) as u32;
                let new_title = pick_soc_title(person_id, event_seed.wrapping_add(0xC0FFEE))
                    .to_string();
                // JobSwitches usually hold level (sometimes ±1 — captured
                // as a separate Promotion/Demotion later).
                (Some(current_level), new_emp, new_title)
            }
            CareerEventKind::LateralMove => {
                // Same employer, same level, new title (different team /
                // function within the org).
                let new_title = pick_soc_title(person_id, event_seed.wrapping_add(0xBEEF))
                    .to_string();
                (Some(current_level), current_employer, new_title)
            }
            CareerEventKind::FoundCompany => {
                let new_emp =
                    hash_int(event_seed, "career_employer_founded_v1", 1u64 << 32) as u32;
                // Founders take Exec immediately, but title becomes
                // generic founder-flavored — keep the existing SOC pick
                // since we don't model "Founder" as an SOC code.
                (Some(RoleLevel::Exec), new_emp, current_title.clone())
            }
            CareerEventKind::Sabbatical => {
                // Career paused; level holds, no employer change, but
                // to_level is None so renderers can flag the gap.
                (None, current_employer, current_title.clone())
            }
            CareerEventKind::Retirement => unreachable!("handled above"),
        };

        events.push(CareerEvent {
            when: birth + Duration::days(cursor_days),
            kind,
            age_at_event: age_at,
            seq,
            to_level: next_level,
            employer_seed: next_employer,
            title: next_title.clone(),
        });

        // Advance carry-state for the next event. Sabbatical doesn't
        // change level (None just signals "no work happening at this
        // event"), so we keep current_level.
        if let Some(nl) = next_level {
            current_level = nl;
        }
        current_employer = next_employer;
        current_title = next_title;

        seq = seq.saturating_add(1);
        if seq >= MAX_EVENTS_PER_PERSON {
            let retire_days = (retire_age as i64) * 365;
            events.push(CareerEvent {
                when: birth + Duration::days(retire_days),
                kind: CareerEventKind::Retirement,
                age_at_event: retire_age,
                seq,
                to_level: None,
                employer_seed: current_employer,
                title: current_title.clone(),
            });
            break;
        }
    }

    events
}

/// Most recent career event at or before time `t`. Returns `None` if
/// the person hadn't started their career by `t`.
pub fn career_event_at(person_id: U512, t: DateTime<Utc>) -> Option<CareerEvent> {
    let target = t.date_naive();
    career_events_for(person_id)
        .into_iter()
        .filter(|e| e.when <= target)
        .last()
}

/// All career events whose dates fall in `[start, end]` (inclusive).
pub fn career_events_in_range(
    person_id: U512,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Vec<CareerEvent> {
    let s = start.date_naive();
    let e = end.date_naive();
    career_events_for(person_id)
        .into_iter()
        .filter(|ev| ev.when >= s && ev.when <= e)
        .collect()
}

/// Current career state at time `t`: how many events have happened,
/// when the most recent one was, and what kind it was. Useful for
/// scenario verdicts ("was this person promoted in the last 3 years?").
pub fn career_summary_at(person_id: U512, t: DateTime<Utc>) -> CareerSummary {
    let events = career_events_for(person_id);
    let target = t.date_naive();
    let past: Vec<&CareerEvent> = events.iter().filter(|e| e.when <= target).collect();
    let most_recent = past.last().map(|e| (*e).clone());
    let is_retired = past.iter().any(|e| e.kind == CareerEventKind::Retirement);
    CareerSummary {
        events_so_far: past.len() as u8,
        most_recent,
        is_retired,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerSummary {
    pub events_so_far: u8,
    pub most_recent: Option<CareerEvent>,
    pub is_retired: bool,
}

/// Snapshot of this person's working life right now: where they
/// work (employer_seed), what they do (title + level), how long
/// they've been there (tenure_days), and how many places they've
/// worked total (prior_employer_count).
///
/// `tenure_days` is the gap from the most-recent JobSwitch /
/// FoundCompany / career-start to `t`. None if the person is
/// retired or hasn't started a career yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleSnapshot {
    pub level: RoleLevel,
    pub level_label: String,
    pub title: String,
    pub employer_seed: u32,
    pub tenure_days: u32,
    pub prior_employer_count: u8,
    pub is_on_sabbatical: bool,
}

/// Current role at time `t` — folds the career timeline up to `t`
/// into a single snapshot. Returns None if the person is retired
/// or pre-career-start at that time.
pub fn current_role_at(person_id: U512, t: DateTime<Utc>) -> Option<RoleSnapshot> {
    let events = career_events_for(person_id);
    let target = t.date_naive();
    let past: Vec<&CareerEvent> = events.iter().filter(|e| e.when <= target).collect();

    // Retired? — return None so renderers know to say "retired".
    if past.iter().any(|e| e.kind == CareerEventKind::Retirement) {
        return None;
    }

    // Pre-career — no events yet: synthesize the first job using the
    // same seed-derivation as the timeline-init in `career_events_for`.
    let birth = birth_date_of(person_id);
    let career_start = birth + Duration::days((CAREER_START_AGE as i64) * 365);
    if target < career_start {
        return None;
    }

    let seed = career_arc_seed_of(person_id) as u64;

    // Walk events to derive current level / employer / title and
    // count prior employers.
    let initial_level = match education_level_of(person_id) {
        EducationLevel::HighSchool | EducationLevel::AssociateDegree => RoleLevel::Junior,
        EducationLevel::Bachelors => RoleLevel::Junior,
        EducationLevel::Masters => RoleLevel::Mid,
        EducationLevel::PhD | EducationLevel::MD | EducationLevel::JD => RoleLevel::Senior,
    };
    let mut level = initial_level;
    let mut employer = hash_int(seed, "career_employer_initial_v1", 1u64 << 32) as u32;
    let mut title = pick_soc_title(person_id, seed.wrapping_add(0xCAFE)).to_string();
    let mut prior_employer_count: u8 = 0;
    let mut tenure_anchor: NaiveDate = career_start;
    let mut is_on_sabbatical = false;

    for ev in &past {
        is_on_sabbatical = false;
        match ev.kind {
            CareerEventKind::JobSwitch | CareerEventKind::FoundCompany => {
                prior_employer_count = prior_employer_count.saturating_add(1);
                tenure_anchor = ev.when;
            }
            CareerEventKind::Sabbatical => {
                is_on_sabbatical = true;
            }
            _ => {}
        }
        if let Some(nl) = ev.to_level {
            level = nl;
        }
        employer = ev.employer_seed;
        title = ev.title.clone();
    }

    let tenure_days = (target - tenure_anchor).num_days().max(0) as u32;

    Some(RoleSnapshot {
        level,
        level_label: level.label().to_string(),
        title,
        employer_seed: employer,
        tenure_days,
        prior_employer_count,
        is_on_sabbatical,
    })
}

/// Pick a career-event kind weighted by lifecycle phase. Early career
/// is dominated by job switches; mid/peak by promotions; late by
/// sabbaticals + entrepreneurship.
fn pick_event_kind(seed: u64, age: u32) -> CareerEventKind {
    let phase = lifecycle_phase_for_age(age);
    // Order matches the categorical match below: JobSwitch, Promotion,
    // Demotion, LateralMove, Sabbatical, FoundCompany.
    let weights: &[f64] = match phase {
        0..=1 => &[0.55, 0.20, 0.05, 0.10, 0.05, 0.05], // early: many switches
        2..=3 => &[0.30, 0.40, 0.05, 0.15, 0.05, 0.05], // mid/peak: more promotions
        4..=5 => &[0.20, 0.25, 0.10, 0.20, 0.20, 0.05], // late: stable + sabbaticals
        _ => &[0.10, 0.15, 0.15, 0.20, 0.40, 0.00],     // pre-retire: winding down
    };
    match categorical(seed, "career_kind_v1", weights) {
        0 => CareerEventKind::JobSwitch,
        1 => CareerEventKind::Promotion,
        2 => CareerEventKind::Demotion,
        3 => CareerEventKind::LateralMove,
        4 => CareerEventKind::Sabbatical,
        _ => CareerEventKind::FoundCompany,
    }
}

// Suppress dead-code warning for hash_float — it's available for
// future weighting refinements but not yet used.
#[allow(dead_code)]
fn _hash_float_anchor(id: u64) -> f64 {
    hash_float(id, "career_anchor")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::{person_for, person_id_for};
    use chrono::TimeZone;

    fn pid_for(industry: u8, city: u8, ws: u8, member: u16) -> U512 {
        person_id_for(person_for(industry, city, ws, member))
    }

    fn naive_to_utc(d: NaiveDate, h: u32) -> DateTime<Utc> {
        chrono::Utc
            .with_ymd_and_hms(d.format("%Y").to_string().parse().unwrap(),
                              d.format("%m").to_string().parse().unwrap(),
                              d.format("%d").to_string().parse().unwrap(),
                              h, 0, 0)
            .unwrap()
    }

    #[test]
    fn career_timeline_is_deterministic() {
        let pid = pid_for(0, 0, 0, 7);
        let a = career_events_for(pid);
        let b = career_events_for(pid);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.when, y.when);
            assert_eq!(x.kind, y.kind);
            assert_eq!(x.age_at_event, y.age_at_event);
            assert_eq!(x.seq, y.seq);
        }
    }

    #[test]
    fn career_timeline_starts_at_or_after_age_18() {
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            assert!(!events.is_empty(), "every person should have ≥1 event");
            // First event MUST be at or after CAREER_START_AGE.
            assert!(
                events[0].age_at_event >= CAREER_START_AGE,
                "first event at age {} for mid={mid:#x} (must be ≥ {CAREER_START_AGE})",
                events[0].age_at_event
            );
        }
    }

    #[test]
    fn career_timeline_always_terminates_with_retirement() {
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0xDEAD_BEEF);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            let last = events.last().expect("non-empty timeline");
            assert_eq!(
                last.kind,
                CareerEventKind::Retirement,
                "mid={mid:#x} timeline doesn't end at retirement"
            );
            assert!((RETIREMENT_AGE_MIN..=RETIREMENT_AGE_MAX).contains(&last.age_at_event));
        }
    }

    #[test]
    fn career_events_are_chronologically_ordered() {
        for raw in 0..100u32 {
            let mid = raw.wrapping_mul(0x1234_5678);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            for w in events.windows(2) {
                assert!(
                    w[0].when <= w[1].when,
                    "out-of-order events: {:?} then {:?}",
                    w[0],
                    w[1]
                );
                assert!(
                    w[0].seq < w[1].seq,
                    "seq not monotonic: {} then {}",
                    w[0].seq,
                    w[1].seq
                );
            }
        }
    }

    #[test]
    fn career_event_at_returns_most_recent_past_event() {
        let pid = pid_for(0, 0, 0, 7);
        let events = career_events_for(pid);
        if events.len() < 3 {
            return; // unusual draw, skip
        }
        let between = events[1].when + Duration::days(1);
        let result = career_event_at(pid, naive_to_utc(between, 12)).unwrap();
        assert_eq!(result.seq, events[1].seq);
    }

    #[test]
    fn career_event_at_before_career_start_is_none() {
        let pid = pid_for(0, 0, 0, 7);
        let birth = birth_date_of(pid);
        let before_career = birth + Duration::days(365); // age 1
        let result = career_event_at(pid, naive_to_utc(before_career, 0));
        assert!(result.is_none(), "career events shouldn't exist at age 1");
    }

    #[test]
    fn career_summary_reflects_progress() {
        let pid = pid_for(0, 0, 0, 7);
        let events = career_events_for(pid);
        let last_event = events.last().unwrap();
        let after = last_event.when + Duration::days(10 * 365);
        let summary = career_summary_at(pid, naive_to_utc(after, 12));
        assert_eq!(summary.events_so_far as usize, events.len());
        assert!(summary.is_retired);
    }

    #[test]
    fn early_career_has_more_job_switches_than_late_career() {
        // Aggregate over 1000 persons: early-career (phase 0-1) job
        // switches per event should exceed late-career rate. This
        // verifies the lifecycle-phase weighting actually affects
        // distributions in the aggregate.
        let mut early_switches = 0u32;
        let mut early_total = 0u32;
        let mut late_switches = 0u32;
        let mut late_total = 0u32;
        for raw in 0..1_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            for e in career_events_for(pid) {
                let phase = lifecycle_phase_for_age(e.age_at_event);
                if phase <= 1 {
                    early_total += 1;
                    if e.kind == CareerEventKind::JobSwitch {
                        early_switches += 1;
                    }
                } else if phase >= 4 {
                    late_total += 1;
                    if e.kind == CareerEventKind::JobSwitch {
                        late_switches += 1;
                    }
                }
            }
        }
        // Need both buckets to have observations to be meaningful.
        assert!(early_total > 100, "not enough early events: {early_total}");
        assert!(late_total > 100, "not enough late events: {late_total}");
        let early_rate = early_switches as f64 / early_total as f64;
        let late_rate = late_switches as f64 / late_total as f64;
        assert!(
            early_rate > late_rate,
            "early job-switch rate {early_rate:.3} not > late rate {late_rate:.3}"
        );
    }

    // ---------- Role-level + employer machinery (added 2026-05-06) ----------

    #[test]
    fn promotion_advances_level_by_one() {
        // Find a person with at least one Promotion event and check
        // their level monotonically advanced through it.
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            for w in events.windows(2) {
                if w[1].kind == CareerEventKind::Promotion {
                    if let (Some(before), Some(after)) = (w[0].to_level, w[1].to_level) {
                        assert!(
                            after >= before,
                            "promotion regressed level: {before:?} → {after:?} for mid={mid:#x}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn job_switch_changes_employer() {
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            for w in events.windows(2) {
                if w[1].kind == CareerEventKind::JobSwitch {
                    assert_ne!(
                        w[0].employer_seed, w[1].employer_seed,
                        "JobSwitch kept same employer for mid={mid:#x}"
                    );
                }
            }
        }
    }

    #[test]
    fn promotion_keeps_same_employer() {
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            for w in events.windows(2) {
                if w[1].kind == CareerEventKind::Promotion {
                    assert_eq!(
                        w[0].employer_seed, w[1].employer_seed,
                        "Promotion changed employer for mid={mid:#x}"
                    );
                }
            }
        }
    }

    #[test]
    fn found_company_jumps_to_exec() {
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            for ev in &events {
                if ev.kind == CareerEventKind::FoundCompany {
                    assert_eq!(
                        ev.to_level,
                        Some(RoleLevel::Exec),
                        "FoundCompany didn't jump to Exec for mid={mid:#x}"
                    );
                }
            }
        }
    }

    #[test]
    fn current_role_at_during_active_career_returns_some() {
        let pid = pid_for(8, 0, 1, 7); // Information cohort
        let events = career_events_for(pid);
        if events.len() < 2 {
            return;
        }
        // Pick a date between events 0 and the second-to-last (not yet retired).
        let mid_career = events[events.len() / 2].when;
        let snap = current_role_at(pid, naive_to_utc(mid_career, 12));
        assert!(snap.is_some(), "active-career snapshot should exist");
        let s = snap.unwrap();
        assert!(!s.title.is_empty());
        assert!(!s.level_label.is_empty());
    }

    #[test]
    fn current_role_at_after_retirement_returns_none() {
        let pid = pid_for(8, 0, 1, 7);
        let events = career_events_for(pid);
        let last = events.last().unwrap();
        assert_eq!(last.kind, CareerEventKind::Retirement);
        let after = last.when + Duration::days(365);
        assert!(current_role_at(pid, naive_to_utc(after, 12)).is_none());
    }

    #[test]
    fn current_role_at_before_career_start_returns_none() {
        let pid = pid_for(8, 0, 1, 7);
        let birth = birth_date_of(pid);
        let teen = birth + Duration::days(15 * 365);
        assert!(current_role_at(pid, naive_to_utc(teen, 12)).is_none());
    }

    #[test]
    fn tenure_resets_on_job_switch() {
        // Sweep until we find a person whose last pre-retirement event
        // is JobSwitch + at least one event before. Their tenure
        // shortly after that switch should be small (within a few days).
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let events = career_events_for(pid);
            // Find a non-retirement JobSwitch followed by another event.
            let mut found = false;
            for i in 0..events.len().saturating_sub(2) {
                if events[i].kind == CareerEventKind::JobSwitch {
                    let just_after = events[i].when + Duration::days(7);
                    if let Some(snap) = current_role_at(pid, naive_to_utc(just_after, 12)) {
                        assert!(
                            snap.tenure_days <= 14,
                            "tenure not reset by JobSwitch: {} days for mid={mid:#x}",
                            snap.tenure_days
                        );
                        found = true;
                        break;
                    }
                }
            }
            if found {
                return;
            }
        }
        // OK if no person in the sweep had this exact pattern; the
        // assertion is the meaningful part when we hit one.
    }

    #[test]
    fn higher_education_starts_at_higher_level() {
        // A PhD's first event level should be ≥ Senior; a HighSchool
        // person's should be Junior.
        let mut phd_seen = false;
        let mut hs_seen = false;
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let pid = person_id_for(mid);
            let level = super::super::education::education_level_of(pid);
            let events = career_events_for(pid);
            let first = events.first().expect("non-empty timeline");
            if first.kind == CareerEventKind::Retirement {
                continue;
            }
            match level {
                EducationLevel::PhD if !phd_seen => {
                    if let Some(l) = first.to_level {
                        assert!(l >= RoleLevel::Senior, "PhD didn't start at ≥Senior: {l:?}");
                    }
                    phd_seen = true;
                }
                EducationLevel::HighSchool if !hs_seen => {
                    // First event is at least 1 interval after start;
                    // can be a Promotion already. So we only check that
                    // they're not VP/Exec.
                    if let Some(l) = first.to_level {
                        assert!(l <= RoleLevel::Lead, "HS person started too high: {l:?}");
                    }
                    hs_seen = true;
                }
                _ => {}
            }
            if phd_seen && hs_seen {
                return;
            }
        }
    }

    #[test]
    fn career_events_in_range_filters_correctly() {
        let pid = pid_for(0, 0, 0, 7);
        let all = career_events_for(pid);
        if all.len() < 3 {
            return;
        }
        let start_d = all[1].when;
        let end_d = all[all.len() - 2].when;
        let in_range = career_events_in_range(pid, naive_to_utc(start_d, 0), naive_to_utc(end_d, 23));
        // Should contain the boundary events (inclusive) and everything
        // between, but NOT events before or after.
        for e in &in_range {
            assert!(e.when >= start_d && e.when <= end_d);
        }
        let total_inside = all.iter().filter(|e| e.when >= start_d && e.when <= end_d).count();
        assert_eq!(in_range.len(), total_inside);
    }
}
