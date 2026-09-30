//! `people` — Service 1.
//!
//! Slot layout: `person_id = (industry:6, city:6, workplace_seed:8, member_idx:12)`.
//! `person_id` IS `mail_id` IS `small_id` — the bridge is an identity
//! cast. Cohort enumeration via pure `Space::find()` bit-pattern
//! pushdown. All other attributes (country, age, gender, name,
//! voice, family) are hash-derived.
//!
//! Per the bit-vs-hash search rule (learning.md): the four bit fields
//! are the only searchable axes. Everything else is descriptive.

pub mod avm;
pub mod career;
pub mod cohort;
pub mod daily_life;
pub mod derive;
pub mod education;
pub mod family;
pub mod geography;
pub mod income;
pub mod languages;
pub mod life_events;
pub mod names;
pub mod slot;
pub mod tables;
pub mod timezones;
pub mod views;

pub use career::{
    career_event_at, career_events_for, career_events_in_range, career_summary_at,
    CareerEvent, CareerEventKind, CareerSummary,
};

pub use avm::PersonAvm;
pub use cohort::{coworkers_of, workplace_members_of, workplace_size_for};
pub use derive::{
    age_of, city_of, country_code_of, country_of, engagement_style_of, family_id_of,
    first_name_of, full_name_of, gender_of, handle_of, industry_of, last_name_of, timezone_of,
    utc_offset_minutes_of, voice_of, EngagementStyle, FullName, VoiceDescriptor,
    VocabularyRegister, MAX_AGE, MIN_AGE,
};
pub use names::Gender;
pub use slot::{
    a_compassion_of, a_respect_of, a_trust_of, age_at, agreeableness_of, archetype_label,
    archetype_of, attachment_anxiety_of, attachment_avoidance_of, birth_date_of,
    build_people_space, c_organization_of, c_productiveness_of, c_responsibility_of,
    career_arc_seed_of, city_idx_of, city_idx_of_mail_id, cognitive_band_of,
    conscientiousness_of, cse_of, dark_flag_of, dt_machiavellianism_of, dt_narcissism_of,
    dt_psychopathy_of, e_assertiveness_of, e_energy_of, e_sociability_of, engagement_band_of,
    extraversion_of, frequent_collab_mail_id_of, frequent_collab_xor_of, hexaco_honesty_of,
    honesty_band_of, industry_idx_of, industry_idx_of_mail_id, life_event_timeline_seed_of,
    lifecycle_epoch_of, lifecycle_phase_at, lifecycle_phase_for_age, lifecycle_phase_of,
    mail_id_of, manager_mail_id_of, manager_xor_of, member_idx_of, member_idx_of_mail_id,
    mentor_mail_id_of, mentor_xor_of, n_anxiety_of, n_depression_of, n_volatility_of,
    network_position_of, neuroticism_of, o_aesthetics_of, o_imagination_of, o_intellect_of,
    openness_of, people_layout, person_for, person_id_for, prosocial_orientation_of, register,
    risk_band_of, risk_tolerance_of, schwartz_conservation_of, schwartz_openness_to_change_of,
    schwartz_self_enhancement_of, schwartz_self_transcendence_of, self_monitor_continuous_of,
    self_monitor_of, simulation_epoch, spouse_mail_id_of, spouse_xor_of, time_discount_of,
    values_quadrant_of, workplace_seed_of, workplace_seed_of_mail_id, MAX_WORKPLACE_SIZE,
    POPULATION_SIZE, W_CITY_IDX, W_INDUSTRY_IDX, W_MEMBER_IDX, W_WORKPLACE_SEED,
};
pub use views::views;

use std::sync::Arc;
use procedural_core::word::U512;
use crate::services::Service;
use crate::views::DynView;

pub struct PeopleService;

impl Service for PeopleService {
    fn name(&self) -> &'static str { "people" }
    fn register_u512(&self, world: &mut procedural_core::world::World<U512>) -> Result<(), procedural_core::world::WorldError> {
        slot::register(world)
    }
    fn views(&self) -> Vec<Arc<dyn DynView>> { views::views() }
}
