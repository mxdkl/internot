//! Bit-layout: widths, offsets, BitLayout factory, Space registration.
//!
//! 32 public BitLayout bits = `(industry:6, city:6, workplace_seed:8,
//! member_idx:12)`. The layout *is* the mail_id; everything beyond
//! bit 32 is private cache populated via `populate_extended`.
//!
//! Tier 1..4 offsets carve the U512 above the public layout:
//!   bits 32..64   — Tier 1 derived bands (cached for fast extract)
//!   bits 64..256  — Tier 2 continuous trait values
//!   bits 256..384 — Tier 3 graph stubs (XOR-encoded neighbors)
//!   bits 384..512 — Tier 4 temporal seeds

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::word::U512;
use procedural_core::world::World;

// ---------- Public 32-bit cohort axes (BitLayout, search-pushdown) ----------

pub const W_MEMBER_IDX: u8 = 12;
pub const W_WORKPLACE_SEED: u8 = 8;
pub const W_CITY_IDX: u8 = 6;
pub const W_INDUSTRY_IDX: u8 = 6;

pub const MAX_WORKPLACE_SIZE: u16 = 1 << W_MEMBER_IDX;
pub const POPULATION_SIZE: u64 = 1u64 << 32;

// ---------- Tier 1 derived-attribute offsets (bits 32..64) ----------

pub const T1_OFFSET_ARCHETYPE: u32 = 32;
pub const T1_WIDTH_ARCHETYPE: u32 = 6;
pub const T1_OFFSET_COGNITIVE: u32 = 38;
pub const T1_WIDTH_COGNITIVE: u32 = 2;
pub const T1_OFFSET_ENGAGEMENT: u32 = 40;
pub const T1_WIDTH_ENGAGEMENT: u32 = 2;
pub const T1_OFFSET_VALUES: u32 = 42;
pub const T1_WIDTH_VALUES: u32 = 2;
pub const T1_OFFSET_RISK: u32 = 44;
pub const T1_WIDTH_RISK: u32 = 2;
pub const T1_OFFSET_LIFECYCLE: u32 = 46;
pub const T1_WIDTH_LIFECYCLE: u32 = 3;
pub const T1_OFFSET_NETWORK: u32 = 49;
pub const T1_WIDTH_NETWORK: u32 = 2;
pub const T1_OFFSET_HONESTY: u32 = 51;
pub const T1_WIDTH_HONESTY: u32 = 2;
pub const T1_OFFSET_DARK_FLAG: u32 = 53;
pub const T1_WIDTH_DARK_FLAG: u32 = 1;
pub const T1_OFFSET_SELF_MONITOR: u32 = 54;
pub const T1_WIDTH_SELF_MONITOR: u32 = 1;
// 9 bits unused (55..64) — reserved for future T1 cache fields.

// ---------- Tier 2: Big Five domain values (bits 64..104) ----------

// 5 domains × 8 bits each. Cluster around archetype_centroid ± 16.

pub const T2_OFFSET_OPENNESS: u32 = 64;
pub const T2_OFFSET_CONSCIENTIOUSNESS: u32 = 72;
pub const T2_OFFSET_EXTRAVERSION: u32 = 80;
pub const T2_OFFSET_AGREEABLENESS: u32 = 88;
pub const T2_OFFSET_NEUROTICISM: u32 = 96;
pub const T2_WIDTH_BIG_FIVE_DOMAIN: u32 = 8;

// ---------- Tier 2: Big Five facets (bits 104..164) ----------

// BFI-2 facets: 3 per domain × 5 domains × 4 bits = 60 bits.
// Each facet seeded from parent domain centroid (>>4 to fit 0..15)
// + ±2 wiggle.

pub const T2_WIDTH_FACET: u32 = 4;

// Openness facets
pub const T2_OFFSET_O_AESTHETICS: u32 = 104;
pub const T2_OFFSET_O_INTELLECT: u32 = 108;
pub const T2_OFFSET_O_IMAGINATION: u32 = 112;

// Conscientiousness facets
pub const T2_OFFSET_C_ORGANIZATION: u32 = 116;
pub const T2_OFFSET_C_PRODUCTIVENESS: u32 = 120;
pub const T2_OFFSET_C_RESPONSIBILITY: u32 = 124;

// Extraversion facets
pub const T2_OFFSET_E_SOCIABILITY: u32 = 128;
pub const T2_OFFSET_E_ASSERTIVENESS: u32 = 132;
pub const T2_OFFSET_E_ENERGY: u32 = 136;

// Agreeableness facets
pub const T2_OFFSET_A_COMPASSION: u32 = 140;
pub const T2_OFFSET_A_RESPECT: u32 = 144;
pub const T2_OFFSET_A_TRUST: u32 = 148;

// Neuroticism facets
pub const T2_OFFSET_N_ANXIETY: u32 = 152;
pub const T2_OFFSET_N_DEPRESSION: u32 = 156;
pub const T2_OFFSET_N_VOLATILITY: u32 = 160;

// ---------- Tier 2: Schwartz 4 higher-order values (bits 164..188) ----------

// 4 quadrants × 6 bits. Argmax becomes T1 values_quadrant.

pub const T2_WIDTH_SCHWARTZ: u32 = 6;
pub const T2_OFFSET_SCHWARTZ_SELF_TRANSCENDENCE: u32 = 164;
pub const T2_OFFSET_SCHWARTZ_SELF_ENHANCEMENT: u32 = 170;
pub const T2_OFFSET_SCHWARTZ_CONSERVATION: u32 = 176;
pub const T2_OFFSET_SCHWARTZ_OPENNESS_TO_CHANGE: u32 = 182;

// ---------- Tier 2: Behavioral economics (bits 188..206) ----------

pub const T2_WIDTH_BEH_ECON: u32 = 6;
pub const T2_OFFSET_RISK_TOLERANCE: u32 = 188;
pub const T2_OFFSET_TIME_DISCOUNT: u32 = 194;
pub const T2_OFFSET_PROSOCIAL: u32 = 200;

// ---------- Tier 2: Attachment (bits 206..218) ----------

// ECR-R 2D: anxiety + avoidance.

pub const T2_WIDTH_ATTACHMENT: u32 = 6;
pub const T2_OFFSET_ATTACHMENT_ANXIETY: u32 = 206;
pub const T2_OFFSET_ATTACHMENT_AVOIDANCE: u32 = 212;

// ---------- Tier 2: Dark Triad (bits 218..230) ----------

// SD3: psychopathy + machiavellianism + narcissism. 4 bits each.
// dark_flag := (psy + mach) ≥ 24 → ~10.9% of population.

pub const T2_WIDTH_DARK_TRIAD: u32 = 4;
pub const T2_OFFSET_DT_PSYCHOPATHY: u32 = 218;
pub const T2_OFFSET_DT_MACHIAVELLIANISM: u32 = 222;
pub const T2_OFFSET_DT_NARCISSISM: u32 = 226;
pub const T1_DARK_FLAG_THRESHOLD: u32 = 24;

// ---------- Tier 2: CSE / HEXACO H / self-monitor continuous (bits 230..246) ----------

pub const T2_OFFSET_CSE: u32 = 230;
pub const T2_WIDTH_CSE: u32 = 6;
pub const T2_OFFSET_HEXACO_H: u32 = 236;
pub const T2_WIDTH_HEXACO_H: u32 = 6;
pub const T2_OFFSET_SELF_MONITOR_C: u32 = 242;
pub const T2_WIDTH_SELF_MONITOR_C: u32 = 4;
// bits 246..256 unused.

// ---------- Tier 3: graph stubs (bits 256..384) ----------

// Each stub stores `neighbor_mail_id XOR self_mail_id`. Recover the
// neighbor by XOR with self's low 32 bits. Constraints (same-workplace,
// same-industry, etc.) are enforced at populate-time; the stub itself
// is just 32 bits of XOR-encoded mail_id.

pub const T3_OFFSET_SPOUSE_XOR: u32 = 256;
pub const T3_OFFSET_MANAGER_XOR: u32 = 288;
pub const T3_OFFSET_MENTOR_XOR: u32 = 320;
pub const T3_OFFSET_FREQUENT_COLLAB_XOR: u32 = 352;
pub const T3_WIDTH_NEIGHBOR_XOR: u32 = 32;

// ---------- Tier 4: temporal seeds (bits 384..512) ----------

// lifecycle_epoch is days-since-simulation-epoch when this person was
// born. Combined with simulation_epoch and `now`, derives age and
// lifecycle phase. The other seeds power per-person career / life
// event timelines (see people::career).

pub const T4_OFFSET_LIFECYCLE_EPOCH: u32 = 384;
pub const T4_WIDTH_LIFECYCLE_EPOCH: u32 = 32;
pub const T4_OFFSET_CAREER_ARC_SEED: u32 = 416;
pub const T4_WIDTH_CAREER_ARC_SEED: u32 = 32;
pub const T4_OFFSET_LIFE_EVENT_SEED: u32 = 448;
pub const T4_WIDTH_LIFE_EVENT_SEED: u32 = 32;
// bits 480..512 unused.

// ---------- BitLayout (32 bits — pushdown-searchable axes only) ----------

pub fn people_layout() -> BitLayout<U512> {
    // BitLayout convention: FIRST-declared field is LSB, LAST-declared
    // is MSB. We want member_idx in the lowest 12 bits (so it varies
    // fastest in `find()` enumeration) and industry_idx in the top 6
    // bits (so industry-narrow queries collapse to one bit-pattern).
    BitLayout::new(vec![
        ("member_idx", W_MEMBER_IDX),
        ("workplace_seed", W_WORKPLACE_SEED),
        ("city_idx", W_CITY_IDX),
        ("industry_idx", W_INDUSTRY_IDX),
    ])
    .expect("people layout fits in 32 bits and within U512")
}

pub fn build_people_space() -> Space<U512> {
    let layout = people_layout();
    let mut s = Space::<U512>::new("people", layout);
    s.indexable_attribute::<u32, _>("industry_idx", "industry_idx", |v| v as u32)
        .expect("register industry_idx");
    s.indexable_attribute::<u32, _>("city_idx", "city_idx", |v| v as u32)
        .expect("register city_idx");
    s.indexable_attribute::<u32, _>("workplace_seed", "workplace_seed", |v| v as u32)
        .expect("register workplace_seed");
    s.indexable_attribute::<u32, _>("member_idx", "member_idx", |v| v as u32)
        .expect("register member_idx");
    s
}

pub fn register(world: &mut World<U512>) -> Result<(), procedural_core::world::WorldError> {
    world.register(build_people_space())
}
