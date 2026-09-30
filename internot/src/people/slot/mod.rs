//! People slot — `person_id` is U512-wide; bottom 32 bits are the
//! public BitLayout (== `mail_id`), upper 480 bits cache derived
//! Tier 1–4 attributes.
//!
//! **Public layout (BitLayout, registered with Space::find()):**
//! 32 bits = `(industry:6, city:6, workplace_seed:8, member_idx:12)`.
//! These 32 bits ARE the mail_id and are pushdown-searchable. The
//! 4 axes are independent dimensions — every (industry, city, ws,
//! member) combination is a unique person.
//!
//! **Internal extended id (U512):** the bottom 32 bits are the mail_id;
//! the upper 480 bits carry cached derived attributes:
//!   - bits 32..64   — Tier 1 derived bands (archetype, cognitive,
//!                      engagement, values_quadrant, risk_band,
//!                      lifecycle_phase, network_position, honesty_band,
//!                      dark_flag, self_monitor)
//!   - bits 64..256  — Tier 2 continuous trait values (Big Five +
//!                      facets, Schwartz, behavioral econ, attachment,
//!                      Dark Triad, CSE, HEXACO H, self-monitor cont.)
//!   - bits 256..384 — Tier 3 graph stubs (XOR-encoded neighbor mail_ids)
//!   - bits 384..512 — Tier 4 temporal seeds (lifecycle/career/life-event)
//!
//! Tier 1 derived bands are CACHED in the U512 for fast extraction
//! (~3ns vs ~30ns hash call) but are NOT independent search axes.
//! Cohort enumeration over derived attributes uses rejection sampling
//! (see `cohort.rs`).
//!
//! Cross-service mail_id stays u32 — every other service references
//! Person via mail_id and is unchanged.
//!
//! See: docs/superpowers/specs/2026-05-04-person-512bit-substrate.md
//!
//! # Module layout
//!
//! - `layout`   — bit-width constants + `BitLayout` + `Space` factory
//!                + `register`
//! - `convert`  — mail_id ↔ U512 (`person_for`, `person_id_for`,
//!                `mail_id_of`, axis extractors)
//! - `temporal` — Tier 4: simulation_epoch, derive_lifecycle_epoch,
//!                age_at, lifecycle_phase_*
//! - `populate` — `populate_extended` (the big function that fills
//!                the U512 from a mail_id)
//! - `tier1`    — Tier 1 derived-band accessors
//! - `tier2`    — Tier 2 continuous trait accessors
//! - `tier3`    — Tier 3 graph-stub accessors
//! - `tier4`    — Tier 4 temporal-seed accessors
//! - `labels`   — archetype label table

pub mod convert;
pub mod labels;
pub mod layout;
pub mod populate;
pub mod temporal;
pub mod tier1;
pub mod tier2;
pub mod tier3;
pub mod tier4;

#[cfg(test)]
mod tests;

// Re-export the full public API at the slot:: path so existing
// `crate::people::slot::name` imports keep working.

pub use convert::{
    city_idx_of, city_idx_of_mail_id, industry_idx_of, industry_idx_of_mail_id, mail_id_of,
    member_idx_of, member_idx_of_mail_id, person_for, person_id_for, workplace_seed_of,
    workplace_seed_of_mail_id,
};
pub use labels::archetype_label;
pub use layout::{
    build_people_space, people_layout, register, MAX_WORKPLACE_SIZE, POPULATION_SIZE,
    W_CITY_IDX, W_INDUSTRY_IDX, W_MEMBER_IDX, W_WORKPLACE_SEED,
};
pub use temporal::{
    age_at, birth_date_of, lifecycle_phase_at, lifecycle_phase_for_age, simulation_epoch,
};
pub use tier1::{
    archetype_of, cognitive_band_of, dark_flag_of, engagement_band_of, honesty_band_of,
    lifecycle_phase_of, network_position_of, risk_band_of, self_monitor_of, values_quadrant_of,
};
pub use tier2::{
    a_compassion_of, a_respect_of, a_trust_of, agreeableness_of, attachment_anxiety_of,
    attachment_avoidance_of, c_organization_of, c_productiveness_of, c_responsibility_of,
    conscientiousness_of, cse_of, dt_machiavellianism_of, dt_narcissism_of, dt_psychopathy_of,
    e_assertiveness_of, e_energy_of, e_sociability_of, extraversion_of, hexaco_honesty_of,
    n_anxiety_of, n_depression_of, n_volatility_of, neuroticism_of, o_aesthetics_of,
    o_imagination_of, o_intellect_of, openness_of, prosocial_orientation_of, risk_tolerance_of,
    schwartz_conservation_of, schwartz_openness_to_change_of, schwartz_self_enhancement_of,
    schwartz_self_transcendence_of, self_monitor_continuous_of, time_discount_of,
};
pub use tier3::{
    frequent_collab_mail_id_of, frequent_collab_xor_of, manager_mail_id_of, manager_xor_of,
    mentor_mail_id_of, mentor_xor_of, spouse_mail_id_of, spouse_xor_of,
};
pub use tier4::{career_arc_seed_of, life_event_timeline_seed_of, lifecycle_epoch_of};
