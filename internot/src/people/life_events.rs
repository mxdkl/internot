//! Life events subsystem — first real consumer of the Tier 4
//! `life_event_timeline_seed`.
//!
//! Produces a deterministic non-career biographical timeline for any
//! person: education graduations, first marriage, children, divorce.
//! Pure function of `(person_id, t)`.
//!
//! Distributions are anchored on real-world stats (US 2020 baseline)
//! so the substrate's lifespan demographics are coherent at scale:
//!
//!   - Median age at first marriage:    ~28 (range 24–34)
//!   - Marriage rate by age 50:         ~80% of population
//!   - Median age at first child:       ~28 (range 22–35)
//!   - Mean children per parent:        ~2 (range 0–4 modeled)
//!   - Divorce rate (of marriages):     ~40%, median 7y in
//!
//! Skipped in v1 (each gated on a scenario that needs it):
//! relocations, bereavements, remarriage, non-marital relationships,
//! adoption.
//!
//! Cross-tier dependencies:
//!   - lifecycle_epoch (Tier 4) — anchors birth/age math
//!   - life_event_timeline_seed (Tier 4) — RNG seed
//!   - education (Tier 4 derived) — graduation event timing + level

use std::sync::OnceLock;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use procedural_core::hash::hash_int;
use procedural_core::word::U512;
use serde::{Deserialize, Serialize};

use super::education::{education_level_of, field_of_study_of, EducationLevel, FieldOfStudy};
use super::family::{child_at, spouse_mail_id_compat_at, FamilyChild};
use super::slot::{birth_date_of, life_event_timeline_seed_of, mail_id_of};

/// Process-wide memo of `person_id → Vec<LifeEvent>`. Same shape
/// as the career cache.
fn life_cache() -> &'static parking_lot::RwLock<std::collections::HashMap<U512, std::sync::Arc<Vec<LifeEvent>>>> {
    static C: OnceLock<parking_lot::RwLock<std::collections::HashMap<U512, std::sync::Arc<Vec<LifeEvent>>>>> = OnceLock::new();
    C.get_or_init(|| parking_lot::RwLock::new(std::collections::HashMap::new()))
}

/// Maximum modeled children per person.
pub const MAX_CHILDREN: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifeEventKind {
    /// Diploma / GED at age 18.
    GraduatedHighSchool,
    /// College graduation. Field of study comes from the education
    /// subsystem's CIP family pick; level is one of Associate /
    /// Bachelors. Renderers can format "BS in Computer Science".
    GraduatedCollege {
        level: EducationLevel,
        field: FieldOfStudy,
    },
    /// Master's, PhD, MD, or JD graduation. Same FieldOfStudy as the
    /// college graduation (substrate models contiguous study tracks).
    GraduatedGraduate {
        level: EducationLevel,
        field: FieldOfStudy,
    },
    /// First marriage. `partner_mail_id` resolves to a real person
    /// in the population (constraint-searched for age/gender match
    /// via `family::spouse_mail_id_compat_at`). One-way reference —
    /// see `family.rs` module docs.
    GotMarried {
        partner_mail_id: u32,
    },
    /// Divorce of the most recent marriage.
    GotDivorced,
    /// Birth of a child. `child_index` is the position within the
    /// parent's child sequence (0 = first, 1 = second). Resolves
    /// via `family::child_at` to either a real adult person or a
    /// minor stub depending on the child's age today.
    HadChild {
        child_index: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeEvent {
    pub when: NaiveDate,
    pub kind: LifeEventKind,
    pub age: u32,
    /// Zero-indexed sequence within this person's timeline.
    pub seq: u8,
}

/// Build the full life-events timeline for this person. Deterministic
/// per `person_id`. Events are emitted in chronological order.
///
/// Memoized via `life_cache` — runs ~6 hash_int per person; called
/// from PersonAvm + life_summary_at + life_event_at.
pub fn life_events_for(person_id: U512) -> Vec<LifeEvent> {
    if let Some(hit) = life_cache().read().get(&person_id).cloned() {
        return (*hit).clone();
    }
    let computed = life_events_for_uncached(person_id);
    life_cache()
        .write()
        .insert(person_id, std::sync::Arc::new(computed.clone()));
    computed
}

fn life_events_for_uncached(person_id: U512) -> Vec<LifeEvent> {
    let seed = life_event_timeline_seed_of(person_id) as u64;
    let birth = birth_date_of(person_id);
    let level = education_level_of(person_id);

    let mut events: Vec<LifeEvent> = Vec::new();

    // ---- Education graduations (deterministic, no probability gate) ----
    // Everyone gets HS graduation at age 18. Note: CompulsorySchool
    // attainment is ~91% in the US; we model 100% for simplicity. If
    // realism demands, a future version can hash-derive a 9% no-HS
    // bucket from Tier 4.

    push_at_age(&mut events, birth, 18, LifeEventKind::GraduatedHighSchool);

    if level != EducationLevel::HighSchool {
        // College graduation — Bachelor's at 22, Associate at 20.
        let field = field_of_study_of(person_id).expect("non-HS has field");
        let (college_level, college_age) = match level {
            EducationLevel::AssociateDegree => (EducationLevel::AssociateDegree, 20),
            // Anyone with Bachelors-or-higher graduates undergrad first.
            _ => (EducationLevel::Bachelors, 22),
        };
        push_at_age(
            &mut events,
            birth,
            college_age,
            LifeEventKind::GraduatedCollege {
                level: college_level,
                field: field.clone(),
            },
        );

        // Graduate degree — only for Masters/PhD/MD/JD.
        if matches!(
            level,
            EducationLevel::Masters
                | EducationLevel::PhD
                | EducationLevel::MD
                | EducationLevel::JD
        ) {
            push_at_age(
                &mut events,
                birth,
                level.typical_grad_age(),
                LifeEventKind::GraduatedGraduate {
                    level,
                    field: field.clone(),
                },
            );
        }
    }

    // ---- Marriage gate (~80% by age 50) ----
    let marries = hash_int(seed, "life_marries_v1", 100) < 80;
    let mut marriage_age: Option<u32> = None;
    if marries {
        // Marriage age: triangular-ish around 28, range 22..38.
        // Compose two uniform draws so the distribution peaks centrally.
        let a = hash_int(seed, "life_marriage_age_a_v1", 17) as u32; // 0..16
        let b = hash_int(seed, "life_marriage_age_b_v1", 17) as u32; // 0..16
        let m_age = 22 + ((a + b) / 2); // 22..38
        marriage_age = Some(m_age);
        // Resolve the spouse to a real population mail_id with age
        // compatibility at the marriage date. Pinned to a date close
        // to the actual marriage so the search is anchored correctly.
        let marriage_when = birth + Duration::days(m_age as i64 * 365);
        use chrono::TimeZone;
        let marriage_dt = Utc
            .with_ymd_and_hms(marriage_when.year(), marriage_when.month(), marriage_when.day(), 12, 0, 0)
            .single()
            .unwrap_or(crate::universe::DEFAULT_NOW());
        let partner_mail_id = spouse_mail_id_compat_at(person_id, marriage_dt);
        push_at_age(
            &mut events,
            birth,
            m_age,
            LifeEventKind::GotMarried { partner_mail_id },
        );
    }

    // ---- Children — modeled only for married persons in v1 ----
    // (v2 could add non-marital births at lower rate.) Each married
    // person draws a child count 0..4. Births spaced 2-4y apart starting
    // 1-3y after marriage.
    let mut last_birth_age: Option<u32> = None;
    if let Some(m_age) = marriage_age {
        let n_children = hash_int(seed, "life_n_children_v1", (MAX_CHILDREN + 1) as u64) as u8;
        // Skew toward 1-2: re-roll if 4. Two-pass keeps distribution
        // ~ {0:25%, 1:25%, 2:25%, 3:13%, 4:12%} → reweighted to
        // ~ {0:30%, 1:25%, 2:25%, 3:12%, 4:8%}. Good enough for v1.
        let n_children = if n_children == 4 {
            let reroll = hash_int(seed, "life_n_children_reroll_v1", 3) as u8;
            std::cmp::min(reroll, MAX_CHILDREN)
        } else {
            n_children
        };
        let mut child_age = m_age + 1 + hash_int(seed, "life_first_child_offset_v1", 3) as u32;
        for i in 0..n_children {
            // Cap at age 45 (substrate doesn't model later parenthood).
            if child_age >= 45 {
                break;
            }
            push_at_age(
                &mut events,
                birth,
                child_age,
                LifeEventKind::HadChild { child_index: i },
            );
            last_birth_age = Some(child_age);
            child_age += 2 + hash_int(seed.wrapping_add(i as u64), "life_child_gap_v1", 3) as u32;
        }
    }

    // ---- Divorce gate (~40% of marriages) ----
    if let Some(m_age) = marriage_age {
        let divorces = hash_int(seed, "life_divorce_v1", 100) < 40;
        if divorces {
            // Divorce age: marriage_age + (5..15y); typical 7-8.
            let gap = 5 + hash_int(seed, "life_divorce_gap_v1", 11) as u32; // 5..15
            let d_age = m_age + gap;
            // Don't divorce after age 70 — keep it within working life.
            // Also don't divorce before the last child's birth (keep
            // the family formation contiguous in the timeline).
            let earliest_divorce = last_birth_age.unwrap_or(m_age) + 1;
            let d_age = d_age.max(earliest_divorce).min(70);
            push_at_age(&mut events, birth, d_age, LifeEventKind::GotDivorced);
        }
    }

    // Final sort — events were emitted in roughly-chronological order
    // but the marriage→children→divorce sequence interleaves. Sort by
    // date, then assign monotonic seq.
    events.sort_by_key(|e| e.when);
    for (i, e) in events.iter_mut().enumerate() {
        e.seq = i as u8;
    }

    events
}

fn push_at_age(events: &mut Vec<LifeEvent>, birth: NaiveDate, age: u32, kind: LifeEventKind) {
    let when = birth + Duration::days(age as i64 * 365);
    events.push(LifeEvent {
        when,
        kind,
        age,
        seq: 0, // placeholder; final sort assigns monotonic seq
    });
}

/// Most recent life event at or before time `t`.
pub fn life_event_at(person_id: U512, t: DateTime<Utc>) -> Option<LifeEvent> {
    let target = t.date_naive();
    life_events_for(person_id)
        .into_iter()
        .filter(|e| e.when <= target)
        .last()
}

/// All life events whose dates fall in `[start, end]` (inclusive).
pub fn life_events_in_range(
    person_id: U512,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Vec<LifeEvent> {
    let s = start.date_naive();
    let e = end.date_naive();
    life_events_for(person_id)
        .into_iter()
        .filter(|ev| ev.when >= s && ev.when <= e)
        .collect()
}

/// Snapshot of marital + family + education state at time `t`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeSummary {
    pub marital_status: MaritalStatus,
    /// Real population mail_id of the current spouse (Married) or
    /// None (NeverMarried / Divorced).
    pub spouse_mail_id: Option<u32>,
    pub children_count: u8,
    /// Resolved children — `FamilyChild::Adult` if their age today
    /// is ≥ 18, else `FamilyChild::Minor` stub. Order matches birth
    /// order on the timeline.
    pub children: Vec<FamilyChild>,
    pub years_since_marriage: Option<u32>,
    pub years_since_divorce: Option<u32>,
    pub highest_education: EducationLevel,
    pub highest_education_label: String,
    pub field_of_study: Option<FieldOfStudy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaritalStatus {
    NeverMarried,
    Married,
    Divorced,
}

pub fn life_summary_at(person_id: U512, t: DateTime<Utc>) -> LifeSummary {
    let events = life_events_for(person_id);
    let target = t.date_naive();
    let past: Vec<&LifeEvent> = events.iter().filter(|e| e.when <= target).collect();

    let mut marital_status = MaritalStatus::NeverMarried;
    let mut spouse_mail_id: Option<u32> = None;
    let mut marriage_when: Option<NaiveDate> = None;
    let mut divorce_when: Option<NaiveDate> = None;
    let mut child_births: Vec<(u8, NaiveDate)> = Vec::new();
    let mut highest_education = EducationLevel::HighSchool;

    for ev in &past {
        match &ev.kind {
            LifeEventKind::GotMarried { partner_mail_id } => {
                marital_status = MaritalStatus::Married;
                spouse_mail_id = Some(*partner_mail_id);
                marriage_when = Some(ev.when);
                divorce_when = None;
            }
            LifeEventKind::GotDivorced => {
                marital_status = MaritalStatus::Divorced;
                spouse_mail_id = None;
                divorce_when = Some(ev.when);
            }
            LifeEventKind::HadChild { child_index } => {
                child_births.push((*child_index, ev.when));
            }
            LifeEventKind::GraduatedHighSchool => {
                if (highest_education as u8) < (EducationLevel::HighSchool as u8) {
                    highest_education = EducationLevel::HighSchool;
                }
            }
            LifeEventKind::GraduatedCollege { level, .. } => {
                highest_education = *level;
            }
            LifeEventKind::GraduatedGraduate { level, .. } => {
                highest_education = *level;
            }
        }
    }

    // Resolve each child birth to a FamilyChild (Adult or Minor)
    // based on their age at the snapshot time.
    let parent_mail_id = mail_id_of(person_id);
    let children: Vec<FamilyChild> = child_births
        .iter()
        .map(|(idx, when)| {
            let age_days = (target - *when).num_days().max(0);
            let target_age_today = (age_days / 365) as u32;
            child_at(parent_mail_id, *idx, target_age_today)
        })
        .collect();
    let children_count = children.len() as u8;

    let years_since_marriage = marriage_when.map(|d| ((target - d).num_days() / 365) as u32);
    let years_since_divorce = divorce_when.map(|d| ((target - d).num_days() / 365) as u32);

    LifeSummary {
        marital_status,
        spouse_mail_id,
        children_count,
        children,
        years_since_marriage,
        years_since_divorce,
        highest_education,
        highest_education_label: highest_education.label().to_string(),
        field_of_study: field_of_study_of(person_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::{person_for, person_id_for};
    use chrono::{Datelike, TimeZone};

    fn pid(industry: u8, city: u8, ws: u8, member: u16) -> U512 {
        person_id_for(person_for(industry, city, ws, member))
    }

    fn naive_to_utc(d: NaiveDate, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(d.year(), d.month(), d.day(), h, 0, 0).unwrap()
    }

    #[test]
    fn timeline_is_deterministic() {
        let p = pid(8, 0, 1, 7);
        let a = life_events_for(p);
        let b = life_events_for(p);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.when, y.when);
            assert_eq!(x.age, y.age);
        }
    }

    #[test]
    fn timeline_includes_high_school_graduation() {
        for raw in 0..200u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            assert!(
                events.iter().any(|e| matches!(e.kind, LifeEventKind::GraduatedHighSchool)),
                "no HS graduation for mid={mid:#x}"
            );
        }
    }

    #[test]
    fn college_grads_have_undergrad_event() {
        for raw in 0..1_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let level = education_level_of(p);
            if level == EducationLevel::HighSchool {
                continue;
            }
            let events = life_events_for(p);
            let has_college = events.iter().any(|e| matches!(e.kind, LifeEventKind::GraduatedCollege { .. }));
            assert!(has_college, "non-HS person missing college event for mid={mid:#x} level={level:?}");
        }
    }

    #[test]
    fn graduate_degrees_have_grad_event() {
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let level = education_level_of(p);
            if !matches!(
                level,
                EducationLevel::Masters | EducationLevel::PhD | EducationLevel::MD | EducationLevel::JD
            ) {
                continue;
            }
            let events = life_events_for(p);
            let has_grad = events.iter().any(|e| matches!(e.kind, LifeEventKind::GraduatedGraduate { .. }));
            assert!(has_grad, "graduate-degree person missing grad event for mid={mid:#x} level={level:?}");
        }
    }

    #[test]
    fn marriage_rate_is_near_80_percent() {
        let mut married = 0u32;
        let n = 5_000u32;
        for raw in 0..n {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            if life_events_for(p).iter().any(|e| matches!(e.kind, LifeEventKind::GotMarried { .. })) {
                married += 1;
            }
        }
        let frac = married as f64 / n as f64;
        assert!(
            (0.72..=0.88).contains(&frac),
            "marriage rate {frac} out of expected ~0.80 band"
        );
    }

    #[test]
    fn marriage_age_is_in_realistic_range() {
        for raw in 0..2_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            for ev in life_events_for(p) {
                if matches!(ev.kind, LifeEventKind::GotMarried { .. }) {
                    assert!(
                        (22..=38).contains(&ev.age),
                        "marriage age {} out of [22, 38] for mid={mid:#x}",
                        ev.age
                    );
                }
            }
        }
    }

    #[test]
    fn divorce_only_after_marriage() {
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            let mut married_seq: Option<u8> = None;
            for ev in &events {
                if matches!(ev.kind, LifeEventKind::GotMarried { .. }) {
                    married_seq = Some(ev.seq);
                }
                if ev.kind == LifeEventKind::GotDivorced {
                    assert!(
                        married_seq.is_some() && married_seq.unwrap() < ev.seq,
                        "divorce without prior marriage for mid={mid:#x}"
                    );
                }
            }
        }
    }

    #[test]
    fn divorce_rate_among_married_is_near_40_percent() {
        let mut married = 0u32;
        let mut divorced = 0u32;
        for raw in 0..5_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            let m = events.iter().any(|e| matches!(e.kind, LifeEventKind::GotMarried { .. }));
            let d = events.iter().any(|e| e.kind == LifeEventKind::GotDivorced);
            if m {
                married += 1;
                if d {
                    divorced += 1;
                }
            }
        }
        let rate = divorced as f64 / married as f64;
        assert!(
            (0.32..=0.48).contains(&rate),
            "divorce rate among married {rate} out of expected ~0.40"
        );
    }

    #[test]
    fn children_are_only_for_married_persons() {
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            let has_child = events.iter().any(|e| matches!(e.kind, LifeEventKind::HadChild { .. }));
            if has_child {
                let has_marriage = events.iter().any(|e| matches!(e.kind, LifeEventKind::GotMarried { .. }));
                assert!(has_marriage, "children without marriage for mid={mid:#x}");
            }
        }
    }

    #[test]
    fn timeline_is_chronological() {
        for raw in 0..500u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            for w in events.windows(2) {
                assert!(w[0].when <= w[1].when, "out-of-order events: {:?} then {:?}", w[0], w[1]);
                assert!(w[0].seq < w[1].seq, "seq not monotonic");
            }
        }
    }

    #[test]
    fn life_summary_reflects_marriage_and_children() {
        // Find a married person with children and verify summary.
        for raw in 0..3_000u32 {
            let mid = raw.wrapping_mul(0x9E37_79B9);
            let p = person_id_for(mid);
            let events = life_events_for(p);
            let has_marriage = events
                .iter()
                .find(|e| matches!(e.kind, LifeEventKind::GotMarried { .. }));
            let n_children = events.iter().filter(|e| matches!(e.kind, LifeEventKind::HadChild { .. })).count();
            if let Some(m_ev) = has_marriage {
                if n_children > 0 {
                    let after_all = events.last().unwrap().when + Duration::days(365 * 5);
                    let summary = life_summary_at(p, naive_to_utc(after_all, 12));
                    assert_eq!(summary.children_count as usize, n_children);
                    if events.iter().any(|e| e.kind == LifeEventKind::GotDivorced) {
                        assert_eq!(summary.marital_status, MaritalStatus::Divorced);
                    } else {
                        assert_eq!(summary.marital_status, MaritalStatus::Married);
                    }
                    assert!(summary.years_since_marriage.is_some());
                    let _ = m_ev;
                    return;
                }
            }
        }
    }

    #[test]
    fn life_summary_before_any_event_is_never_married() {
        let p = pid(8, 0, 1, 7);
        let birth = birth_date_of(p);
        let young = birth + Duration::days(10 * 365);
        let summary = life_summary_at(p, naive_to_utc(young, 12));
        assert_eq!(summary.marital_status, MaritalStatus::NeverMarried);
        assert_eq!(summary.children_count, 0);
    }

    #[test]
    fn life_event_at_returns_most_recent_past_event() {
        let p = pid(8, 0, 1, 7);
        let events = life_events_for(p);
        if events.len() < 2 {
            return;
        }
        let between = events[0].when + Duration::days(365);
        let result = life_event_at(p, naive_to_utc(between, 12));
        assert!(result.is_some());
        // Should be the HS graduation (event 0) since we're 1y after it.
        let r = result.unwrap();
        assert!(r.when <= between);
    }
}
