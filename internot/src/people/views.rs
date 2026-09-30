//! Views the people service exposes.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::universe::Universe;
use crate::views::{DynView, View, ViewError};

use super::avm::PersonAvm;

// ----- read_person -----

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadPersonParams {
    /// The mail_id of the person to read. Same as person_id and
    /// small_id — they're identical in the slot layout.
    pub mail_id: u32,
}

pub struct ReadPerson;

impl View for ReadPerson {
    type Params = ReadPersonParams;
    type Output = PersonAvm;
    const NAME: &'static str = "read_person";
    const DESCRIPTION: &'static str =
        "Look up a person by mail_id. Returns name, handle, age, gender, country, city, industry, and voice descriptors.";

    fn execute(&self, _ctx: &Universe, p: ReadPersonParams) -> Result<PersonAvm, ViewError> {
        Ok(PersonAvm::for_mail_id(p.mail_id))
    }
}

// ----- list_people_in_workplace -----
//
// Pure bit-pattern pushdown via Space::find on (industry_idx, city_idx,
// workplace_seed). Filtered to populated members (member_idx <
// workplace_size). This is the canonical "cohort enumeration" the
// slot architecture exists to make cheap.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPeopleInWorkplaceParams {
    /// Industry slot index (0..=63). Choose from the 16-entry
    /// `INDUSTRIES` table.
    pub industry_idx: u8,
    /// City slot index (0..=63). Choose from the 16-entry `CITIES`
    /// row for the country.
    pub city_idx: u8,
    /// Workplace slot index (0..=255).
    pub workplace_seed: u8,
}

#[derive(Debug, Serialize)]
pub struct ListPeopleInWorkplaceOutput {
    pub workplace: WorkplaceRef,
    pub members: Vec<PersonAvm>,
}

#[derive(Debug, Serialize)]
pub struct WorkplaceRef {
    pub industry_idx: u8,
    pub city_idx: u8,
    pub workplace_seed: u8,
    pub size: u16,
}

pub struct ListPeopleInWorkplace;

impl View for ListPeopleInWorkplace {
    type Params = ListPeopleInWorkplaceParams;
    type Output = ListPeopleInWorkplaceOutput;
    const NAME: &'static str = "list_people_in_workplace";
    const DESCRIPTION: &'static str =
        "Enumerate the people who belong to a workplace cohort, identified by (industry_idx, city_idx, workplace_seed). Pure bit-pattern pushdown — O(workplace_size).";

    fn execute(
        &self,
        ctx: &Universe,
        p: ListPeopleInWorkplaceParams,
    ) -> Result<Self::Output, ViewError> {
        use super::slot::{member_idx_of, person_for, MAX_WORKPLACE_SIZE};
        use procedural_core::sampler::lognormal;

        // Procedural workplace size: lognormal(mu=2.0, sigma=1.5) →
        // median ~7, p99 ~125, capped at MAX_WORKPLACE_SIZE - 1.
        // Keyed on the workplace bits, so the same workplace always
        // has the same size.
        let workplace_seed_u64 =
            (p.industry_idx as u64) << 14 | (p.city_idx as u64) << 8 | p.workplace_seed as u64;
        let raw = lognormal(workplace_seed_u64, "workplace_size_v1", 2.0, 1.5);
        let size = raw
            .max(1.0)
            .min((MAX_WORKPLACE_SIZE - 1) as f64) as u16;

        let space = ctx
            .world_u512
            .space("people")
            .map_err(|e| ViewError::Internal(format!("people space: {e}")))?;

        let candidates = space
            .find()
            .where_eq("industry_idx", p.industry_idx as u64)
            .where_eq("city_idx", p.city_idx as u64)
            .where_eq("workplace_seed", p.workplace_seed as u64)
            .scan_budget(MAX_WORKPLACE_SIZE as usize)
            .execute();

        let members: Vec<PersonAvm> = candidates
            .into_iter()
            .filter(|id| member_idx_of(*id) < size)
            .map(|id| PersonAvm::for_mail_id(crate::people::mail_id_of(id)))
            .collect();

        // Force the canonical id construction is reachable.
        let _ = person_for;

        Ok(ListPeopleInWorkplaceOutput {
            workplace: WorkplaceRef {
                industry_idx: p.industry_idx,
                city_idx: p.city_idx,
                workplace_seed: p.workplace_seed,
                size,
            },
            members,
        })
    }
}

// ----- find_people -----
//
// Cohort search with optional Tier 1 pushdown narrowing + post-filter
// over the cached derived bands. The pushdown axes (industry_idx,
// city_idx) are independent dimensions queryable via Space::find().
// The band predicates (engagement, lifecycle_phase, values_quadrant,
// dark_flag, cognitive_band, risk_band, honesty_band, network_position)
// are hash-derived from the mail_id (cached in T1 bits for ~3ns
// extract); they're not pushdown axes but are O(1) per candidate.
//
// Falls back to a sequential mail_id sweep when neither pushdown axis
// is provided — useful for "find any high-narcissism contributor" type
// queries that don't want to fix a cohort up front.

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct FindPeopleParams {
    /// Optional pushdown narrowing on industry slot index (0..=63).
    pub industry_idx: Option<u8>,
    /// Optional pushdown narrowing on city slot index (0..=63).
    pub city_idx: Option<u8>,
    /// Optional engagement style filter:
    /// "lurker" | "casual" | "contributor" | "influencer".
    pub engagement: Option<String>,
    /// Optional lifecycle phase filter (0..=7).
    /// 0=student/early-twenties .. 7=long-retired.
    pub lifecycle_phase: Option<u8>,
    /// Optional Schwartz values quadrant filter (0..=3).
    /// 0=Self-Transcendence, 1=Self-Enhancement,
    /// 2=Conservation, 3=Openness-to-Change.
    pub values_quadrant: Option<u8>,
    /// Optional Dark Triad top-decile filter.
    pub dark_flag: Option<bool>,
    /// Optional cognitive band filter (0=low..3=exceptional).
    pub cognitive_band: Option<u8>,
    /// Optional risk band filter (0=low..3=extreme).
    pub risk_band: Option<u8>,
    /// Optional language filter — ISO 639-3 code. Returns only people
    /// who speak this language at ≥ Conversational (e.g. "ita", "kor",
    /// "eng"). Useful for "find a Spanish-speaking finance director".
    pub speaks: Option<String>,
    /// Optional hobby category filter: "competitive" | "general" |
    /// "collection" | "observation" | "educational". Returns only people
    /// with at least one hobby in that category.
    pub hobby_category: Option<String>,
    /// Max members to return. Default 20, capped at 200.
    pub limit: Option<u16>,
    /// Max candidates to evaluate before giving up. Default 100_000,
    /// capped at 5_000_000.
    pub scan_budget: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct FindPeopleOutput {
    pub matched: Vec<PersonAvm>,
    pub evaluated: usize,
    pub truncated: bool,
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
        None => String::new(),
    }
}

pub struct FindPeople;

impl View for FindPeople {
    type Params = FindPeopleParams;
    type Output = FindPeopleOutput;
    const NAME: &'static str = "find_people";
    const DESCRIPTION: &'static str = "Search the population for people matching cohort + Tier 1 band predicates. Optional pushdown on (industry_idx, city_idx); optional band filters on engagement, lifecycle_phase, values_quadrant, dark_flag, cognitive_band, risk_band. Returns up to `limit` PersonAvms plus the number of candidates evaluated. Use this for cohort-driven scenarios like 'find a peak-career contributor in Information industry'.";

    fn execute(&self, ctx: &Universe, p: FindPeopleParams) -> Result<Self::Output, ViewError> {
        use super::derive::{engagement_style_of, EngagementStyle};
        use super::slot::{
            cognitive_band_of, dark_flag_of, lifecycle_phase_of, mail_id_of, person_id_for,
            risk_band_of, values_quadrant_of,
        };

        let limit = p.limit.unwrap_or(20).min(200) as usize;
        let scan_budget = p.scan_budget.unwrap_or(100_000).min(5_000_000);

        let want_engagement = match p.engagement.as_deref() {
            None => None,
            Some("lurker") => Some(EngagementStyle::Lurker),
            Some("casual") => Some(EngagementStyle::Casual),
            Some("contributor") => Some(EngagementStyle::Contributor),
            Some("influencer") => Some(EngagementStyle::Influencer),
            Some(other) => {
                return Err(ViewError::Internal(format!(
                    "unknown engagement {other:?}; want lurker|casual|contributor|influencer"
                )));
            }
        };

        let predicate = |pid: procedural_core::word::U512| -> bool {
            if let Some(want) = want_engagement {
                if engagement_style_of(pid) != want {
                    return false;
                }
            }
            if let Some(lp) = p.lifecycle_phase {
                if lifecycle_phase_of(pid) != lp {
                    return false;
                }
            }
            if let Some(vq) = p.values_quadrant {
                if values_quadrant_of(pid) != vq {
                    return false;
                }
            }
            if let Some(df) = p.dark_flag {
                if dark_flag_of(pid) != df {
                    return false;
                }
            }
            if let Some(cb) = p.cognitive_band {
                if cognitive_band_of(pid) != cb {
                    return false;
                }
            }
            if let Some(rb) = p.risk_band {
                if risk_band_of(pid) != rb {
                    return false;
                }
            }
            if let Some(ref code) = p.speaks {
                use super::languages::speaks;
                if !speaks(pid, code) {
                    return false;
                }
            }
            if let Some(ref hc) = p.hobby_category {
                use super::daily_life::hobbies_of;
                let hc_normalized = capitalize_first(hc);
                if !hobbies_of(pid).iter().any(|h| h.category == hc_normalized) {
                    return false;
                }
            }
            true
        };

        let mut matched: Vec<PersonAvm> = Vec::with_capacity(limit);
        let mut evaluated = 0usize;

        if p.industry_idx.is_some() || p.city_idx.is_some() {
            // Pushdown path — use Space::find() with the narrowing axes.
            // Typestate forces us to build the chain in one expression.
            let space = ctx
                .world_u512
                .space("people")
                .map_err(|e| ViewError::Internal(format!("people space: {e}")))?;
            let candidates = match (p.industry_idx, p.city_idx) {
                (Some(ii), Some(ci)) => space
                    .find()
                    .where_eq("industry_idx", ii as u64)
                    .where_eq("city_idx", ci as u64)
                    .scan_budget(scan_budget)
                    .execute(),
                (Some(ii), None) => space
                    .find()
                    .where_eq("industry_idx", ii as u64)
                    .scan_budget(scan_budget)
                    .execute(),
                (None, Some(ci)) => space
                    .find()
                    .where_eq("city_idx", ci as u64)
                    .scan_budget(scan_budget)
                    .execute(),
                (None, None) => unreachable!("guarded above"),
            };
            for raw_id in candidates.into_iter() {
                evaluated += 1;
                // find() returns U512 values with only the BitLayout
                // fields populated — upper-tier cached bands AND the
                // hash-derived attrs (which key on the full U512) all
                // read garbage from zero bits. Reconstruct the full
                // populated id before evaluating the predicate.
                let pid = person_id_for(mail_id_of(raw_id));
                if predicate(pid) {
                    matched.push(PersonAvm::for_mail_id(mail_id_of(pid)));
                    if matched.len() == limit {
                        break;
                    }
                }
            }
        } else {
            // No pushdown — sweep mail_ids sequentially. Hash-jittered
            // start so repeated calls don't all return the same prefix.
            let start = (procedural_core::hash::hash_int(
                (limit as u64) ^ (scan_budget as u64),
                "find_people_sweep_start_v1",
                super::slot::POPULATION_SIZE,
            )) as u32;
            let mut mid = start;
            for _ in 0..scan_budget {
                evaluated += 1;
                let pid = person_id_for(mid);
                if predicate(pid) {
                    matched.push(PersonAvm::for_mail_id(mid));
                    if matched.len() == limit {
                        break;
                    }
                }
                mid = mid.wrapping_add(1);
            }
        }

        let truncated = matched.len() < limit && evaluated >= scan_budget;
        Ok(FindPeopleOutput {
            matched,
            evaluated,
            truncated,
        })
    }
}

// ----- read_person_at_time -----
//
// Surfaces the Tier 4 temporal functions (`age_at`, `lifecycle_phase_at`)
// so agents can reason about a person's age / phase at any point in
// time — not just `DEFAULT_NOW`. This is the missing piece for any
// scenario that asks "what was X's career phase in 2018?" or
// "how old will X be in 2035?".
//
// Birth date never changes; the snapshot AVM `read_person` already
// surfaces it. This view returns ONLY the time-varying projections
// plus the input mail_id — keep it lean since most callers don't need
// a full PersonAvm at every time anchor.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadPersonAtTimeParams {
    pub mail_id: u32,
    /// Time anchor as ISO 8601 / RFC 3339 (e.g. "2030-01-15T00:00:00Z").
    /// For dates without time, append "T00:00:00Z".
    pub when: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadPersonAtTimeOutput {
    pub mail_id: u32,
    pub when: String,
    pub age: u32,
    pub lifecycle_phase: u8,
    pub lifecycle_phase_label: &'static str,
    pub birth_date: String,
}

pub struct ReadPersonAtTime;

impl View for ReadPersonAtTime {
    type Params = ReadPersonAtTimeParams;
    type Output = ReadPersonAtTimeOutput;
    const NAME: &'static str = "read_person_at_time";
    const DESCRIPTION: &'static str = "Project a person's age and lifecycle_phase to an arbitrary time anchor (ISO 8601). Use for temporal-coherence queries like 'what was X's career phase 5 years ago' or 'how old will X be in 2035'. Birth date never changes — for snapshot info at the simulation's default `now`, use `read_person`.";

    fn execute(
        &self,
        _ctx: &Universe,
        p: ReadPersonAtTimeParams,
    ) -> Result<Self::Output, ViewError> {
        use chrono::DateTime;
        use super::slot::{age_at, birth_date_of, lifecycle_phase_at, person_id_for};
        let when = DateTime::parse_from_rfc3339(&p.when)
            .map_err(|e| {
                ViewError::Internal(format!(
                    "could not parse `when` as RFC3339 / ISO 8601: {e}; \
                     example: 2030-01-15T00:00:00Z"
                ))
            })?
            .with_timezone(&chrono::Utc);
        let pid = person_id_for(p.mail_id);
        let phase = lifecycle_phase_at(pid, when);
        const LABELS: [&str; 8] = [
            "student/early-twenties",
            "early-career",
            "mid-career",
            "peak-career",
            "late-career",
            "semi-retired",
            "retired",
            "long-retired",
        ];
        Ok(ReadPersonAtTimeOutput {
            mail_id: p.mail_id,
            when: when.to_rfc3339(),
            age: age_at(pid, when),
            lifecycle_phase: phase,
            lifecycle_phase_label: LABELS[phase as usize % 8],
            birth_date: birth_date_of(pid).format("%Y-%m-%d").to_string(),
        })
    }
}

// ----- read_career_history -----
//
// Surfaces the career arc subsystem (first real consumer of the
// Tier 4 `career_arc_seed`). Returns the deterministic per-person
// career timeline plus a summary at the queried time anchor — agents
// can ask "did this person change jobs in the last 5 years?" or
// "when does this person retire?".

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadCareerHistoryParams {
    pub mail_id: u32,
    /// Optional time anchor for the summary (ISO 8601 / RFC 3339).
    /// Defaults to DEFAULT_NOW.
    pub when: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadCareerHistoryOutput {
    pub mail_id: u32,
    pub when: String,
    pub events: Vec<crate::people::career::CareerEvent>,
    pub summary: crate::people::career::CareerSummary,
}

pub struct ReadCareerHistory;

impl View for ReadCareerHistory {
    type Params = ReadCareerHistoryParams;
    type Output = ReadCareerHistoryOutput;
    const NAME: &'static str = "read_career_history";
    const DESCRIPTION: &'static str = "Return a person's full deterministic career timeline (job switches, promotions, demotions, lateral moves, sabbaticals, founding, retirement) plus a summary at the queried time anchor. Use for any 'when did X happen' / 'is X retired' / 'has X switched jobs recently' question. Pairs with find_people for cohort-driven career-arc scenarios.";

    fn execute(
        &self,
        ctx: &Universe,
        p: ReadCareerHistoryParams,
    ) -> Result<Self::Output, ViewError> {
        use chrono::DateTime;
        use crate::people::{career_events_for, career_summary_at, person_id_for};
        let when = if let Some(s) = p.when.as_deref() {
            DateTime::parse_from_rfc3339(s)
                .map_err(|e| ViewError::Internal(format!(
                    "could not parse `when` as RFC3339: {e}; example: 2030-01-15T00:00:00Z"
                )))?
                .with_timezone(&chrono::Utc)
        } else {
            ctx.now
        };
        let pid = person_id_for(p.mail_id);
        Ok(ReadCareerHistoryOutput {
            mail_id: p.mail_id,
            when: when.to_rfc3339(),
            events: career_events_for(pid),
            summary: career_summary_at(pid, when),
        })
    }
}

// ----- exported aggregator -----

pub fn views() -> Vec<Arc<dyn DynView>> {
    vec![
        Arc::new(ReadPerson),
        Arc::new(ListPeopleInWorkplace),
        Arc::new(FindPeople),
        Arc::new(ReadPersonAtTime),
        Arc::new(ReadCareerHistory),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::universe::Universe;

    #[test]
    fn read_person_returns_avm() {
        let u = Universe::new();
        let v = ReadPerson;
        let out = View::execute(&v, &u, ReadPersonParams { mail_id: 8197 }).unwrap();
        assert_eq!(out.mail_id, 8197);
        assert!(!out.name_first.is_empty());
    }

    // ---------- read_career_history ----------

    #[test]
    fn read_career_history_returns_timeline_and_summary() {
        let u = Universe::new();
        let out = View::execute(
            &ReadCareerHistory,
            &u,
            ReadCareerHistoryParams { mail_id: 8197, when: None },
        )
        .unwrap();
        assert_eq!(out.mail_id, 8197);
        assert!(!out.events.is_empty(), "every person has at least 1 event");
        // Last event must be retirement.
        assert_eq!(
            out.events.last().unwrap().kind,
            crate::people::CareerEventKind::Retirement
        );
        // Summary's events_so_far ≤ total events.
        assert!((out.summary.events_so_far as usize) <= out.events.len());
    }

    #[test]
    fn read_career_history_at_future_anchor_shows_more_progress() {
        let u = Universe::new();
        let now = View::execute(
            &ReadCareerHistory,
            &u,
            ReadCareerHistoryParams {
                mail_id: 8197,
                when: Some("2025-06-30T09:00:00Z".into()),
            },
        )
        .unwrap();
        let future = View::execute(
            &ReadCareerHistory,
            &u,
            ReadCareerHistoryParams {
                mail_id: 8197,
                when: Some("2050-06-30T09:00:00Z".into()),
            },
        )
        .unwrap();
        assert_eq!(now.events.len(), future.events.len(),
                   "the canonical timeline doesn't change with the anchor");
        assert!(future.summary.events_so_far >= now.summary.events_so_far,
                "future summary should have ≥ current events_so_far");
        assert!(future.summary.is_retired,
                "anchor 25y in future must show retirement");
    }

    // ---------- read_person_at_time ----------

    #[test]
    fn read_person_at_time_default_now_matches_read_person() {
        // Picking the default-now anchor must yield the same age + phase
        // values as the snapshot view returns. This is the key
        // cross-view consistency check.
        let u = Universe::new();
        for raw in 0..50u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let snap = View::execute(&ReadPerson, &u, ReadPersonParams { mail_id: mid }).unwrap();
            let out = View::execute(
                &ReadPersonAtTime,
                &u,
                ReadPersonAtTimeParams {
                    mail_id: mid,
                    when: "2025-06-30T09:00:00Z".to_string(), // = DEFAULT_NOW
                },
            )
            .unwrap();
            assert_eq!(out.age, snap.age, "mid={mid:#x} age drift");
            assert_eq!(out.lifecycle_phase, snap.lifecycle_phase, "mid={mid:#x} phase drift");
            assert_eq!(out.birth_date, snap.birth_date, "mid={mid:#x} bd drift");
        }
    }

    #[test]
    fn read_person_at_time_ages_forward_5y() {
        let u = Universe::new();
        let mid = 8197u32;
        let now = View::execute(
            &ReadPersonAtTime,
            &u,
            ReadPersonAtTimeParams {
                mail_id: mid,
                when: "2025-06-30T09:00:00Z".into(),
            },
        )
        .unwrap();
        let later = View::execute(
            &ReadPersonAtTime,
            &u,
            ReadPersonAtTimeParams {
                mail_id: mid,
                when: "2030-06-30T09:00:00Z".into(),
            },
        )
        .unwrap();
        assert_eq!(later.age, now.age + 5, "5 years should add 5 years of age");
        assert!(later.lifecycle_phase >= now.lifecycle_phase);
        assert_eq!(later.birth_date, now.birth_date, "birth date is invariant");
    }

    #[test]
    fn read_person_at_time_rejects_malformed_when() {
        let u = Universe::new();
        let err = View::execute(
            &ReadPersonAtTime,
            &u,
            ReadPersonAtTimeParams {
                mail_id: 1,
                when: "yesterday".into(),
            },
        );
        assert!(err.is_err());
    }

    // ---------- find_people ----------

    #[test]
    fn find_people_with_no_filters_returns_default_limit() {
        let u = Universe::new();
        let v = FindPeople;
        let out = View::execute(&v, &u, FindPeopleParams {
            industry_idx: None,
            city_idx: None,
            engagement: None,
            lifecycle_phase: None,
            values_quadrant: None,
            dark_flag: None,
            cognitive_band: None,
            risk_band: None,
            limit: None,
            speaks: None,
            hobby_category: None,
            scan_budget: None,
        }).unwrap();
        assert_eq!(out.matched.len(), 20, "default limit should fill");
        assert!(out.evaluated >= 20);
        assert!(!out.truncated);
    }

    #[test]
    fn find_people_pushdown_constrains_to_industry_city() {
        let u = Universe::new();
        let v = FindPeople;
        let out = View::execute(&v, &u, FindPeopleParams {
            industry_idx: Some(3),
            city_idx: Some(5),
            engagement: None,
            lifecycle_phase: None,
            values_quadrant: None,
            dark_flag: None,
            cognitive_band: None,
            risk_band: None,
            limit: Some(50),
            speaks: None,
            hobby_category: None,
            scan_budget: Some(200_000),
        }).unwrap();
        assert!(!out.matched.is_empty());
        for p in &out.matched {
            let pid = super::super::slot::person_id_for(p.mail_id);
            assert_eq!(super::super::slot::industry_idx_of(pid), 3);
            assert_eq!(super::super::slot::city_idx_of(pid), 5);
        }
    }

    #[test]
    fn find_people_with_dark_flag_filter_returns_only_dark_flag() {
        let u = Universe::new();
        let v = FindPeople;
        let out = View::execute(&v, &u, FindPeopleParams {
            industry_idx: None,
            city_idx: None,
            engagement: None,
            lifecycle_phase: None,
            values_quadrant: None,
            dark_flag: Some(true),
            cognitive_band: None,
            risk_band: None,
            limit: Some(10),
            speaks: None,
            hobby_category: None,
            scan_budget: Some(500),
        }).unwrap();
        // Dark flag is ~10% so 500 candidates should yield ~50 hits;
        // capped at limit=10.
        assert!(!out.matched.is_empty(), "expected ≥1 dark_flag match in 500 candidates");
        for p in &out.matched {
            assert!(p.dark_flag, "non-dark person leaked through filter");
        }
    }

    #[test]
    fn find_people_with_engagement_filter_respects_engagement() {
        let u = Universe::new();
        let v = FindPeople;
        let out = View::execute(&v, &u, FindPeopleParams {
            industry_idx: None,
            city_idx: None,
            engagement: Some("contributor".into()),
            lifecycle_phase: None,
            values_quadrant: None,
            dark_flag: None,
            cognitive_band: None,
            risk_band: None,
            limit: Some(5),
            speaks: None,
            hobby_category: None,
            scan_budget: Some(2_000),
        }).unwrap();
        assert!(!out.matched.is_empty(), "expected ≥1 contributor in 2K candidates (~12% rate)");
        for p in &out.matched {
            assert_eq!(p.engagement, "contributor");
        }
    }

    #[test]
    fn find_people_compound_filter_yields_intersect() {
        let u = Universe::new();
        let v = FindPeople;
        let out = View::execute(&v, &u, FindPeopleParams {
            industry_idx: Some(0), // Information industry
            city_idx: None,
            engagement: Some("contributor".into()),
            lifecycle_phase: Some(3), // peak-career
            values_quadrant: None,
            dark_flag: None,
            cognitive_band: None,
            risk_band: None,
            limit: Some(5),
            speaks: None,
            hobby_category: None,
            scan_budget: Some(500_000),
        }).unwrap();
        // ~12% contributor × ~16% lifecycle phase 3 = ~1.9% of the industry
        // slice. With 500K scan budget the scan should reach the limit=5.
        assert!(
            !out.matched.is_empty(),
            "compound filter returned 0 matches in 500K candidates — \
             this means the pushdown predicate path is reading garbage \
             from unpopulated U512 bits"
        );
        for p in &out.matched {
            let pid = super::super::slot::person_id_for(p.mail_id);
            assert_eq!(super::super::slot::industry_idx_of(pid), 0);
            assert_eq!(p.engagement, "contributor");
            assert_eq!(p.lifecycle_phase, 3);
        }
    }

    #[test]
    fn find_people_unknown_engagement_errors() {
        let u = Universe::new();
        let v = FindPeople;
        let err = View::execute(&v, &u, FindPeopleParams {
            industry_idx: None,
            city_idx: None,
            engagement: Some("celebrity".into()),
            lifecycle_phase: None,
            values_quadrant: None,
            dark_flag: None,
            cognitive_band: None,
            risk_band: None,
            limit: None,
            speaks: None,
            hobby_category: None,
            scan_budget: None,
        });
        assert!(err.is_err());
    }

    #[test]
    fn list_people_in_workplace_returns_members() {
        let u = Universe::new();
        let v = ListPeopleInWorkplace;
        // Sweep workplace slots until we find one with at least one
        // member. Most workplaces have median ~7 members, so finding
        // one populated is near-certain in the first few seeds.
        for seed in 0..32u8 {
            let out = View::execute(
                &v,
                &u,
                ListPeopleInWorkplaceParams {
                    industry_idx: 0,
                    city_idx: 0,
                    workplace_seed: seed,
                },
            )
            .unwrap();
            if !out.members.is_empty() {
                // Every member should belong to the queried slot.
                for m in &out.members {
                    let pid = super::super::slot::person_id_for(m.mail_id);
                    assert_eq!(super::super::slot::industry_idx_of(pid), 0);
                    assert_eq!(super::super::slot::city_idx_of(pid), 0);
                    assert_eq!(super::super::slot::workplace_seed_of(pid), seed);
                }
                return;
            }
        }
        panic!("no populated workplace found in first 32 seeds");
    }
}
