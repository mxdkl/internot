//! `PersonAvm` — the Attribute-Value Matrix for one person.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::universe::DEFAULT_NOW;

use super::career::{current_role_at, RoleSnapshot};
use super::daily_life::{chronotype_of, hobbies_of, working_hours_of, Hobby, WorkingHours};
use super::derive::{
    age_of, city_of, country_of, engagement_style_of, full_name_of, gender_of, handle_of,
    industry_of, timezone_of, utc_offset_minutes_of, voice_of, EngagementStyle, FullName,
    VocabularyRegister,
};
use super::education::{education_profile_of, EducationProfile};
use super::family::{parent_mail_id_of, reciprocal_spouse_of};
use super::geography::{location_snapshot_of_at, LocationSnapshot};
use super::income::{income_snapshot_of_at, IncomeSnapshot};
use super::languages::{languages_spoken_of, Language};
use super::life_events::{life_summary_at, LifeSummary};
use super::names::Gender;
use super::slot::{
    agreeableness_of, archetype_label, archetype_of, attachment_anxiety_of,
    attachment_avoidance_of, birth_date_of, cognitive_band_of, conscientiousness_of, dark_flag_of,
    dt_machiavellianism_of, dt_narcissism_of, dt_psychopathy_of, extraversion_of,
    frequent_collab_mail_id_of, honesty_band_of, lifecycle_phase_of, manager_mail_id_of,
    mentor_mail_id_of, network_position_of, neuroticism_of, openness_of, person_id_for,
    risk_band_of, self_monitor_of, spouse_mail_id_of, values_quadrant_of,
};

#[derive(Debug, Clone, Serialize)]
pub struct PersonAvm {
    pub mail_id: u32,
    pub name_first: String,
    pub name_last: String,
    pub handle: String,
    pub age: u32,
    pub gender: &'static str,
    pub country: &'static str,
    pub city: &'static str,
    pub industry: &'static str,
    /// IANA timezone name (e.g. "Europe/Rome"). Derived from country.
    pub timezone: &'static str,
    /// UTC offset in minutes (e.g. 60 for Europe/Rome). Convenient for
    /// arithmetic on working-hours or quiet-hours windows.
    pub utc_offset_minutes: i32,
    pub voice_formality: f32,
    pub voice_verbosity: f32,
    pub voice_vocabulary: &'static str,
    /// Posting/commenting activity tier. Renderers use this for things
    /// like "Power User" badges, "new here" hints, or follower-count
    /// ordering. Categorical: lurker / casual / contributor / influencer.
    pub engagement: &'static str,
    /// Big Five archetype cluster index (0-63). T1 derived attribute.
    pub personality_archetype: u8,
    /// Human-readable archetype label, e.g. "Steady Curious Outgoing Ally".
    pub personality_label: &'static str,
    /// Cognitive band: 0=low, 1=mid, 2=high, 3=exceptional.
    pub cognitive_band: u8,
    /// Schwartz higher-order values quadrant: 0=Self-Transcendence,
    /// 1=Self-Enhancement, 2=Conservation, 3=Openness-to-Change.
    pub values_quadrant: u8,
    /// Risk tolerance band: 0=low, 1=mid, 2=high, 3=extreme.
    pub risk_band: u8,
    /// Lifecycle phase 0..7 (student, early-career, ..., post-retired).
    pub lifecycle_phase: u8,
    /// Network position band: 0=lurker, 1=member, 2=connector, 3=hub.
    pub network_position: u8,
    /// HEXACO Honesty-Humility band: 0=low, 1=mid, 2=high, 3=exceptional.
    pub honesty_band: u8,
    /// True if this person scores in the top decile of Dark Triad
    /// (psychopathy + Machiavellianism). Use for adversarial-cohort
    /// scenario seeding.
    pub dark_flag: bool,
    /// Self-monitoring band (0=low, 1=high). Predicts brokerage /
    /// network-centrality propensity (Fang et al. 2015).
    pub self_monitor: u8,
    /// Big Five Openness (0-255). Cached Tier 2 bits seeded by
    /// archetype: archetype's O bit = 1 → value ≥ 128.
    pub openness: u8,
    /// Big Five Conscientiousness (0-255). Same archetype-banded scheme.
    pub conscientiousness: u8,
    /// Big Five Extraversion (0-255).
    pub extraversion: u8,
    /// Big Five Agreeableness (0-255).
    pub agreeableness: u8,
    /// Big Five Neuroticism (0-255). 4-banded by archetype's top 2 bits.
    pub neuroticism: u8,
    // ---- Phase 2C continuous traits (Schwartz quadrant lives in
    //      values_quadrant above; Schwartz 4 continuous values are
    //      reachable via slot::schwartz_*_of for callers who want them).
    /// Adult attachment anxiety (ECR-R, 0-63).
    pub attachment_anxiety: u8,
    /// Adult attachment avoidance (ECR-R, 0-63).
    pub attachment_avoidance: u8,
    /// Dark Triad — psychopathy (SD3, 0-15).
    pub dt_psychopathy: u8,
    /// Dark Triad — Machiavellianism (SD3, 0-15).
    pub dt_machiavellianism: u8,
    /// Dark Triad — narcissism (SD3, 0-15).
    pub dt_narcissism: u8,
    // ---- Tier 3 graph stubs — recovered neighbor mail_ids. Each is
    //      a u32 from `xor ^ self_mail_id`; the candidate is constrained
    //      to a plausible cohort (see slot.rs T3 docs). Reciprocity is
    //      a graph-spec concern and not yet enforced.
    pub spouse_mail_id: u32,
    pub manager_mail_id: u32,
    pub mentor_mail_id: u32,
    pub frequent_collab_mail_id: u32,
    // ---- Tier 4 temporal seeds — birth date is the human-friendly
    //      projection of `lifecycle_epoch`. Other temporal accessors
    //      (age_at, lifecycle_phase_at) are functions of (id, t), so
    //      they don't appear in this snapshot AVM.
    /// Birth date in ISO 8601 (YYYY-MM-DD).
    pub birth_date: String,
    /// Education profile: highest degree, field of study, graduation
    /// year, cohort industry. Time-invariant per person.
    pub education: EducationProfile,
    /// Current working role at the snapshot time. None if the person
    /// is retired or pre-career-start. Carries level, title (SOC),
    /// employer_seed, tenure, prior employer count.
    pub current_role: Option<RoleSnapshot>,
    /// Marital + family + highest-education snapshot at the snapshot
    /// time. NeverMarried / Married / Divorced; spouse_seed if
    /// applicable; children_count; years-since milestones.
    pub life_summary: LifeSummary,
    /// Sleep/work rhythm: Morning / Intermediate / Evening. Drives
    /// working hours and influences calendar coherence.
    pub chronotype: &'static str,
    /// Country-baseline working hours shifted by chronotype.
    pub working_hours: WorkingHours,
    /// 2–5 hobbies / interests, weighted by Big Five + age.
    pub hobbies: Vec<Hobby>,
    /// Languages spoken: every native language of the cohort country
    /// + acquired English with proficiency drawn from EF EPI bands.
    pub languages: Vec<Language>,
    /// Birth city + current city + years-in-current-city. ~30% of
    /// the population was born in a different city than where they
    /// live now.
    pub location: LocationSnapshot,
    /// mail_id of the mother (parent_index 0) — None if she'd be
    /// outside the substrate's MAX_AGE window. Resolves to a real
    /// PersonAvm.
    pub mother_mail_id: Option<u32>,
    /// mail_id of the father (parent_index 1) — same caveat.
    pub father_mail_id: Option<u32>,
    /// mail_id of the spouse IFF the marriage is reciprocal
    /// (spouse(A)=B AND spouse(B)=A). One-way "spouse_mail_id" lives
    /// on `life_summary`; this field is the stricter check.
    pub reciprocal_spouse_mail_id: Option<u32>,
    /// Income band derived from career level + industry + country
    /// cost-of-living. Five buckets Low → VeryHigh.
    pub income: IncomeSnapshot,
}

impl PersonAvm {
    /// Build the AVM at the canonical `DEFAULT_NOW`. Most callers
    /// want this — the time-varying fields (current_role,
    /// life_summary) snapshot at the substrate's default anchor.
    pub fn for_mail_id(mail_id: u32) -> Self {
        Self::for_mail_id_at(mail_id, DEFAULT_NOW())
    }

    /// Build the AVM with the time-varying fields pinned to a
    /// specific moment. Use this when scenarios sweep across years
    /// (e.g. "Eli's role 5 years ago").
    pub fn for_mail_id_at(mail_id: u32, t: DateTime<Utc>) -> Self {
        let pid = person_id_for(mail_id);
        let name: FullName = full_name_of(pid);
        let v = voice_of(pid);
        PersonAvm {
            mail_id,
            name_first: name.first.to_string(),
            name_last: name.last.to_string(),
            handle: handle_of(pid),
            age: age_of(pid),
            gender: match gender_of(pid) {
                Gender::Male => "male",
                Gender::Female => "female",
            },
            country: country_of(pid),
            city: city_of(pid),
            industry: industry_of(pid),
            timezone: timezone_of(pid),
            utc_offset_minutes: utc_offset_minutes_of(pid),
            voice_formality: v.formality_baseline,
            voice_verbosity: v.verbosity,
            voice_vocabulary: match v.vocabulary {
                VocabularyRegister::Casual => "casual",
                VocabularyRegister::Neutral => "neutral",
                VocabularyRegister::Formal => "formal",
                VocabularyRegister::Technical => "technical",
            },
            engagement: match engagement_style_of(pid) {
                EngagementStyle::Lurker => "lurker",
                EngagementStyle::Casual => "casual",
                EngagementStyle::Contributor => "contributor",
                EngagementStyle::Influencer => "influencer",
            },
            personality_archetype: archetype_of(pid),
            personality_label: archetype_label(archetype_of(pid)),
            cognitive_band: cognitive_band_of(pid),
            values_quadrant: values_quadrant_of(pid),
            risk_band: risk_band_of(pid),
            lifecycle_phase: lifecycle_phase_of(pid),
            network_position: network_position_of(pid),
            honesty_band: honesty_band_of(pid),
            dark_flag: dark_flag_of(pid),
            self_monitor: self_monitor_of(pid),
            openness: openness_of(pid),
            conscientiousness: conscientiousness_of(pid),
            extraversion: extraversion_of(pid),
            agreeableness: agreeableness_of(pid),
            neuroticism: neuroticism_of(pid),
            attachment_anxiety: attachment_anxiety_of(pid),
            attachment_avoidance: attachment_avoidance_of(pid),
            dt_psychopathy: dt_psychopathy_of(pid),
            dt_machiavellianism: dt_machiavellianism_of(pid),
            dt_narcissism: dt_narcissism_of(pid),
            spouse_mail_id: spouse_mail_id_of(pid),
            manager_mail_id: manager_mail_id_of(pid),
            mentor_mail_id: mentor_mail_id_of(pid),
            frequent_collab_mail_id: frequent_collab_mail_id_of(pid),
            birth_date: birth_date_of(pid).format("%Y-%m-%d").to_string(),
            education: education_profile_of(pid),
            current_role: current_role_at(pid, t),
            life_summary: life_summary_at(pid, t),
            chronotype: chronotype_of(pid).label(),
            working_hours: working_hours_of(pid),
            hobbies: hobbies_of(pid),
            languages: languages_spoken_of(pid),
            location: location_snapshot_of_at(pid, t),
            mother_mail_id: parent_mail_id_of(pid, 0),
            father_mail_id: parent_mail_id_of(pid, 1),
            reciprocal_spouse_mail_id: reciprocal_spouse_of(pid),
            income: income_snapshot_of_at(pid, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avm_is_deterministic() {
        let a = PersonAvm::for_mail_id(8197);
        let b = PersonAvm::for_mail_id(8197);
        assert_eq!(a.name_first, b.name_first);
        assert_eq!(a.handle, b.handle);
    }

    #[test]
    fn avm_handles_diverse_locales() {
        // Sweep across workplaces (small_id += 4096 advances workplace_seed
        // since member_idx is the LSB 12 bits).
        let mut seen_countries = std::collections::HashSet::new();
        for ws in 0..8u32 {
            for mi in 0..4u32 {
                let avm = PersonAvm::for_mail_id((ws << 12) | mi);
                seen_countries.insert(avm.country);
            }
        }
        assert!(
            seen_countries.len() >= 3,
            "expected diverse countries; saw {:?}",
            seen_countries
        );
    }
}
