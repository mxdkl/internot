//! `populate_extended` — fill the U512 above bit 32 with cached
//! Tier 1 derived bands, Tier 2 continuous trait values, Tier 3
//! graph stubs, and Tier 4 temporal seeds.
//!
//! Hot path: called from `convert::person_id_for(mail_id)`, which
//! every cross-service join goes through.
//!
//! Coherence invariants enforced here:
//! - T1 lifecycle_phase = lifecycle_phase_for_age(age_at(id, DEFAULT_NOW))
//! - T1 values_quadrant = argmax of the four Schwartz values
//! - T1 risk_band      = T2 risk_tolerance >> 4
//! - T1 honesty_band   = T2 hexaco_h >> 4
//! - T1 dark_flag      = (T2 psychopathy + T2 machiavellianism) >= 24
//! - T1 self_monitor   = T2 self_monitor_continuous >= 8
//!
//! The cache and its source-of-truth function therefore can never
//! disagree.

use procedural_core::hash::hash_int;
use procedural_core::sampler::categorical;
use procedural_core::word::{BitWord, U512};

use crate::universe::DEFAULT_NOW;

use super::layout::*;
use super::temporal::{age_at, derive_lifecycle_epoch, lifecycle_phase_for_age};

/// Returns the centroid OCEAN values for an archetype, in 8-bit
/// (0..255) scale. Tier 2 domain bits cluster around these centroids
/// (±16 wiggle).
const fn archetype_centroid(archetype: u8) -> [u8; 5] {
    // Binary OCEA centroids: centers of [0,127] and [128,255] bands.
    // With ±16 wiggle, values land safely inside their band: low →
    // 47..79 ⊂ [0,127], high → 175..207 ⊂ [128,255].
    let o = if archetype & 1 != 0 { 191 } else { 63 };
    let c = if archetype & 2 != 0 { 191 } else { 63 };
    let e = if archetype & 4 != 0 { 191 } else { 63 };
    let a = if archetype & 8 != 0 { 191 } else { 63 };
    // 4-level N centroids: centers of 64-wide bands at offsets 32, 96,
    // 160, 224. With ±16 wiggle: 16..48, 80..112, 144..176, 208..240
    // — all safely inside their respective band.
    let n_band = (archetype >> 4) & 3;
    let n = 32 + n_band * 64;
    [o, c, e, a, n]
}

pub fn populate_extended(id: &mut U512, mail_id: u32) {
    let m = mail_id as u64;

    // ---- Tier 1: archetype + cognitive + engagement.

    let archetype = hash_int(m, "t1_archetype_v1", 1u64 << T1_WIDTH_ARCHETYPE);
    *id = id.insert_bits(T1_OFFSET_ARCHETYPE, T1_WIDTH_ARCHETYPE, archetype);

    let cognitive = hash_int(m, "t1_cognitive_v1", 1u64 << T1_WIDTH_COGNITIVE);
    *id = id.insert_bits(T1_OFFSET_COGNITIVE, T1_WIDTH_COGNITIVE, cognitive);

    // Engagement band must match the EngagementStyle categorical
    // distribution (50% Lurker / 35% Casual / 12% Contributor / 3%
    // Influencer) — same hash key as derive::engagement_style_of so
    // the T1 bit and the categorical accessor always agree.
    const ENGAGEMENT_WEIGHTS: &[f64] = &[0.50, 0.35, 0.12, 0.03];
    let engagement = categorical(m, "engagement_style_v1", ENGAGEMENT_WEIGHTS) as u64;
    *id = id.insert_bits(T1_OFFSET_ENGAGEMENT, T1_WIDTH_ENGAGEMENT, engagement);

    // values_quadrant / risk_band / honesty_band / dark_flag / self_monitor
    // are populated below from their Tier 2 sources so the cache and the
    // continuous values can never disagree.

    // ---- Tier 3 graph stubs — XOR-encoded partial neighbor mail_ids.
    //      Each stub recovers a 32-bit candidate neighbor mail_id
    //      constrained to a plausible cohort.

    let self_low32 = mail_id as u64;

    // Spouse — random member, different family. Same-family check is a
    // graph-spec concern.
    let spouse_neighbor = hash_int(m, "t3_spouse_neighbor_v1", 1u64 << 32);
    *id = id.insert_bits(
        T3_OFFSET_SPOUSE_XOR,
        T3_WIDTH_NEIGHBOR_XOR,
        spouse_neighbor ^ self_low32,
    );

    // Manager — same workplace, different member_idx.
    let self_member_idx = (mail_id & 0xFFF) as u64;
    let mgr_member = {
        let raw = hash_int(m, "t3_manager_member_v1", 1u64 << W_MEMBER_IDX);
        if raw == self_member_idx {
            (raw + 1) & 0xFFF
        } else {
            raw
        }
    };
    let workplace_upper = (mail_id & 0xFFFF_F000) as u64;
    let manager_neighbor = workplace_upper | mgr_member;
    *id = id.insert_bits(
        T3_OFFSET_MANAGER_XOR,
        T3_WIDTH_NEIGHBOR_XOR,
        manager_neighbor ^ self_low32,
    );

    // Mentor — same industry, different city.
    let self_industry = ((mail_id >> 26) & 0x3F) as u64;
    let self_city = ((mail_id >> 20) & 0x3F) as u64;
    let mentor_city = {
        let raw = hash_int(m, "t3_mentor_city_v1", 1u64 << W_CITY_IDX);
        if raw == self_city {
            (raw + 1) & 0x3F
        } else {
            raw
        }
    };
    let mentor_ws = hash_int(m, "t3_mentor_ws_v1", 1u64 << W_WORKPLACE_SEED);
    let mentor_member = hash_int(m, "t3_mentor_member_v1", 1u64 << W_MEMBER_IDX);
    let mentor_neighbor =
        (self_industry << 26) | (mentor_city << 20) | (mentor_ws << 12) | mentor_member;
    *id = id.insert_bits(
        T3_OFFSET_MENTOR_XOR,
        T3_WIDTH_NEIGHBOR_XOR,
        mentor_neighbor ^ self_low32,
    );

    // Frequent collaborator — same workplace, different member_idx,
    // and different from manager_member.
    let collab_member = {
        let raw = hash_int(m, "t3_collab_member_v1", 1u64 << W_MEMBER_IDX);
        let mut adjusted = raw;
        if adjusted == self_member_idx {
            adjusted = (adjusted + 1) & 0xFFF;
        }
        if adjusted == mgr_member {
            adjusted = (adjusted + 1) & 0xFFF;
        }
        if adjusted == self_member_idx {
            adjusted = (adjusted + 1) & 0xFFF;
        }
        adjusted
    };
    let collab_neighbor = workplace_upper | collab_member;
    *id = id.insert_bits(
        T3_OFFSET_FREQUENT_COLLAB_XOR,
        T3_WIDTH_NEIGHBOR_XOR,
        collab_neighbor ^ self_low32,
    );

    // ---- Tier 4 seeds. Populated BEFORE Tier 1 lifecycle_phase so
    //      the cached T1 phase can be derived from
    //      `lifecycle_epoch + DEFAULT_NOW`.

    let lifecycle_epoch = derive_lifecycle_epoch(mail_id);
    *id = id.insert_bits(T4_OFFSET_LIFECYCLE_EPOCH, T4_WIDTH_LIFECYCLE_EPOCH, lifecycle_epoch);
    let career_arc = hash_int(m, "t4_career_arc_seed_v1", 1u64 << 32);
    *id = id.insert_bits(T4_OFFSET_CAREER_ARC_SEED, T4_WIDTH_CAREER_ARC_SEED, career_arc);
    let life_event = hash_int(m, "t4_life_event_seed_v1", 1u64 << 32);
    *id = id.insert_bits(T4_OFFSET_LIFE_EVENT_SEED, T4_WIDTH_LIFE_EVENT_SEED, life_event);

    // T1 lifecycle_phase = bit-cached snapshot of
    // lifecycle_phase_at(id, DEFAULT_NOW). Computed from the same
    // lifecycle_epoch the temporal accessor reads, so the cache and
    // the function never disagree.
    let cached_phase = lifecycle_phase_for_age(age_at(*id, DEFAULT_NOW())) as u64;
    *id = id.insert_bits(T1_OFFSET_LIFECYCLE, T1_WIDTH_LIFECYCLE, cached_phase);

    let network = hash_int(m, "t1_network_v1", 1u64 << T1_WIDTH_NETWORK);
    *id = id.insert_bits(T1_OFFSET_NETWORK, T1_WIDTH_NETWORK, network);

    // ---- Tier 2: Big Five domains seeded by archetype centroid + ±16 wiggle.

    let arch = (id.extract_bits(T1_OFFSET_ARCHETYPE, T1_WIDTH_ARCHETYPE)) as u8;
    let [o_c, c_c, e_c, a_c, n_c] = archetype_centroid(arch);
    let domains = [
        (T2_OFFSET_OPENNESS, "t2_o_v1", o_c),
        (T2_OFFSET_CONSCIENTIOUSNESS, "t2_c_v1", c_c),
        (T2_OFFSET_EXTRAVERSION, "t2_e_v1", e_c),
        (T2_OFFSET_AGREEABLENESS, "t2_a_v1", a_c),
        (T2_OFFSET_NEUROTICISM, "t2_n_v1", n_c),
    ];
    for (offset, key, centroid) in domains {
        let wiggle = hash_int(m, key, 33) as i32 - 16; // -16..=+16
        let value = (centroid as i32 + wiggle).clamp(0, 255) as u64;
        *id = id.insert_bits(offset, T2_WIDTH_BIG_FIVE_DOMAIN, value);
    }

    // ---- Tier 2: Big Five facets (15 × 4 bits). Each facet centroid
    //      is (parent_domain_value / 16) + ±2 hash wiggle.

    let facets: [(u32, &str, u8); 15] = [
        (T2_OFFSET_O_AESTHETICS,    "t2_facet_o_aesthetics_v1",   o_c),
        (T2_OFFSET_O_INTELLECT,     "t2_facet_o_intellect_v1",    o_c),
        (T2_OFFSET_O_IMAGINATION,   "t2_facet_o_imagination_v1",  o_c),
        (T2_OFFSET_C_ORGANIZATION,    "t2_facet_c_organization_v1",    c_c),
        (T2_OFFSET_C_PRODUCTIVENESS,  "t2_facet_c_productiveness_v1",  c_c),
        (T2_OFFSET_C_RESPONSIBILITY,  "t2_facet_c_responsibility_v1",  c_c),
        (T2_OFFSET_E_SOCIABILITY,    "t2_facet_e_sociability_v1",    e_c),
        (T2_OFFSET_E_ASSERTIVENESS,  "t2_facet_e_assertiveness_v1",  e_c),
        (T2_OFFSET_E_ENERGY,         "t2_facet_e_energy_v1",         e_c),
        (T2_OFFSET_A_COMPASSION,  "t2_facet_a_compassion_v1",  a_c),
        (T2_OFFSET_A_RESPECT,     "t2_facet_a_respect_v1",     a_c),
        (T2_OFFSET_A_TRUST,       "t2_facet_a_trust_v1",       a_c),
        (T2_OFFSET_N_ANXIETY,     "t2_facet_n_anxiety_v1",     n_c),
        (T2_OFFSET_N_DEPRESSION,  "t2_facet_n_depression_v1",  n_c),
        (T2_OFFSET_N_VOLATILITY,  "t2_facet_n_volatility_v1",  n_c),
    ];
    for (offset, key, parent_centroid) in facets {
        // Re-scale 0..255 domain centroid to 0..15 facet centroid (>>4).
        let facet_centroid = (parent_centroid >> 4) as i32;
        let wiggle = hash_int(m, key, 5) as i32 - 2; // -2..=+2
        let value = (facet_centroid + wiggle).clamp(0, 15) as u64;
        *id = id.insert_bits(offset, T2_WIDTH_FACET, value);
    }

    // ---- Tier 2 Phase 2C — Schwartz / behavioral / attachment / DT
    //      / CSE / HEXACO H / self-monitor continuous, with T1 cache
    //      derivation.

    // Schwartz 4 higher-order: hash four independent 6-bit values;
    // T1 values_quadrant cache stores argmax. Independent keys make
    // each marginal uniform; argmax is therefore approximately
    // uniform across the 4 quadrants.
    let schwartz_keys = [
        ("t2_schwartz_self_transcendence_v1", T2_OFFSET_SCHWARTZ_SELF_TRANSCENDENCE),
        ("t2_schwartz_self_enhancement_v1",   T2_OFFSET_SCHWARTZ_SELF_ENHANCEMENT),
        ("t2_schwartz_conservation_v1",       T2_OFFSET_SCHWARTZ_CONSERVATION),
        ("t2_schwartz_openness_to_change_v1", T2_OFFSET_SCHWARTZ_OPENNESS_TO_CHANGE),
    ];
    let mut schwartz_values = [0u64; 4];
    for (i, (key, offset)) in schwartz_keys.iter().enumerate() {
        let v = hash_int(m, key, 1u64 << T2_WIDTH_SCHWARTZ);
        schwartz_values[i] = v;
        *id = id.insert_bits(*offset, T2_WIDTH_SCHWARTZ, v);
    }
    let mut argmax_idx = 0usize;
    for i in 1..4 {
        if schwartz_values[i] > schwartz_values[argmax_idx] {
            argmax_idx = i;
        }
    }
    *id = id.insert_bits(T1_OFFSET_VALUES, T1_WIDTH_VALUES, argmax_idx as u64);

    // Risk tolerance (6 bits) → T1 risk_band (2 bits) = >> 4.
    let risk_tolerance = hash_int(m, "t2_risk_tolerance_v1", 1u64 << T2_WIDTH_BEH_ECON);
    *id = id.insert_bits(T2_OFFSET_RISK_TOLERANCE, T2_WIDTH_BEH_ECON, risk_tolerance);
    *id = id.insert_bits(T1_OFFSET_RISK, T1_WIDTH_RISK, risk_tolerance >> 4);

    // Time discount and prosocial orientation — no T1 cache.
    let time_discount = hash_int(m, "t2_time_discount_v1", 1u64 << T2_WIDTH_BEH_ECON);
    *id = id.insert_bits(T2_OFFSET_TIME_DISCOUNT, T2_WIDTH_BEH_ECON, time_discount);
    let prosocial = hash_int(m, "t2_prosocial_v1", 1u64 << T2_WIDTH_BEH_ECON);
    *id = id.insert_bits(T2_OFFSET_PROSOCIAL, T2_WIDTH_BEH_ECON, prosocial);

    // Attachment (ECR-R 2D) — no T1 cache; both 6 bits.
    let att_anxiety = hash_int(m, "t2_attachment_anxiety_v1", 1u64 << T2_WIDTH_ATTACHMENT);
    *id = id.insert_bits(T2_OFFSET_ATTACHMENT_ANXIETY, T2_WIDTH_ATTACHMENT, att_anxiety);
    let att_avoidance = hash_int(m, "t2_attachment_avoidance_v1", 1u64 << T2_WIDTH_ATTACHMENT);
    *id = id.insert_bits(T2_OFFSET_ATTACHMENT_AVOIDANCE, T2_WIDTH_ATTACHMENT, att_avoidance);

    // Dark Triad (SD3) — 4 bits each. T1 dark_flag = (psy+mach) >= threshold.
    let psychopathy = hash_int(m, "t2_dt_psychopathy_v1", 1u64 << T2_WIDTH_DARK_TRIAD);
    *id = id.insert_bits(T2_OFFSET_DT_PSYCHOPATHY, T2_WIDTH_DARK_TRIAD, psychopathy);
    let machiavellianism = hash_int(m, "t2_dt_machiavellianism_v1", 1u64 << T2_WIDTH_DARK_TRIAD);
    *id = id.insert_bits(T2_OFFSET_DT_MACHIAVELLIANISM, T2_WIDTH_DARK_TRIAD, machiavellianism);
    let narcissism = hash_int(m, "t2_dt_narcissism_v1", 1u64 << T2_WIDTH_DARK_TRIAD);
    *id = id.insert_bits(T2_OFFSET_DT_NARCISSISM, T2_WIDTH_DARK_TRIAD, narcissism);
    let dark_flag_v = if (psychopathy + machiavellianism) as u32 >= T1_DARK_FLAG_THRESHOLD {
        1u64
    } else {
        0
    };
    *id = id.insert_bits(T1_OFFSET_DARK_FLAG, T1_WIDTH_DARK_FLAG, dark_flag_v);

    // CSE (Core Self-Evaluations) — no T1 cache.
    let cse = hash_int(m, "t2_cse_v1", 1u64 << T2_WIDTH_CSE);
    *id = id.insert_bits(T2_OFFSET_CSE, T2_WIDTH_CSE, cse);

    // HEXACO Honesty-Humility (6 bits) → T1 honesty_band (2 bits) = >> 4.
    let hexaco_h = hash_int(m, "t2_hexaco_h_v1", 1u64 << T2_WIDTH_HEXACO_H);
    *id = id.insert_bits(T2_OFFSET_HEXACO_H, T2_WIDTH_HEXACO_H, hexaco_h);
    *id = id.insert_bits(T1_OFFSET_HONESTY, T1_WIDTH_HONESTY, hexaco_h >> 4);

    // Self-monitor continuous (4 bits) → T1 self_monitor (1 bit) = high half.
    let sm_continuous = hash_int(m, "t2_self_monitor_continuous_v1", 1u64 << T2_WIDTH_SELF_MONITOR_C);
    *id = id.insert_bits(T2_OFFSET_SELF_MONITOR_C, T2_WIDTH_SELF_MONITOR_C, sm_continuous);
    let sm_t1 = if sm_continuous >= 8 { 1u64 } else { 0 };
    *id = id.insert_bits(T1_OFFSET_SELF_MONITOR, T1_WIDTH_SELF_MONITOR, sm_t1);
}
