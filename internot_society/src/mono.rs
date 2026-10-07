//! The monotone world (`thinking/claude/004`–`006`): kinship laid out so
//! that the counts a search asks for are closed form.
//!
//! People live in **cells**: one per heritage group of the pack (or one cell
//! for everyone in a heritage-blind world). Within a cell, every coupling
//! between index spaces preserves order, so a prefix in one space is a
//! prefix in the next:
//! - **first unions** pair each wife cohort's wives (by union age) with an
//!   exact interleave of husband cohorts ([`procedural_core::lattice::ExactInterleave`]), whose counts
//!   are an integer table (wife cohort × husband cohort) filled in year
//!   order under each husband cohort's adult men, by the pack's age-gap
//!   kernel. Each group is its own union space, and an **open market**
//!   pairs a share of every group's wives and men across groups
//!   (`research/2026-10-03-monotone-cells-math.md`);
//! - **deaths** are quantile slots of **classes**: rank intervals of one
//!   cell cohort and sex whose members share one requirement (alive at 16).
//!   A class's slots run through one affine permutation ([`AffinePerm`]),
//!   so "dead by t" in a class is one quantile count;
//! - **births** choose mothers among the cell's women alive through the
//!   year (survival first), so no death depends on a birth; a child's group
//!   is its mother's.
//!
//! Not yet: re-partnering, migration, same-sex unions, areas, kin repair.

use procedural_core::curve::{log_logistic_cdf, truncated_sigmoid_cdf};
use procedural_core::key::Key;
use procedural_core::lattice::{InterleaveTable, RationalBeatty};
use procedural_core::life::{HazardTable, Siler};
use procedural_core::partition::{mul_div, mul_div_ceil, SystematicShares};
use procedural_core::perm::{AffinePerm, Bijection, CompactPerm4};
use procedural_core::quantile::{count_from_threshold, slot_w, OctaveShape, OctaveTable};
use procedural_core::stream::year_start;
use rayon::prelude::*;

use crate::params::{Heritage, Params, Sex};

mod education;
mod household;
mod naming;
mod residence;
mod schools;
mod work;
pub use education::{EducationPath, Level as EducationLevel, Schooling};
pub use schools::{Institution, InstitutionInfo, Stint};
pub use work::{Employer, EmployerInfo, Inactive, Job, WorkSpell};
pub use household::{Household, Members};
pub use naming::{Middle, Surname};
pub use residence::{state_abbr, Dwelling, Home, Info as UnitInfo, Places, Pos, Source as UnitSource, Unit, AREA, CLUSTER, COUNTY, LEVELS, TRACT, ZONE};

const YEAR: f64 = 31_556_952.0;
/// Half a mean year in seconds (`(YEAR / 2) as i64`): a cohort's `mid` is
/// its year's start plus this.
const HALF_YEAR: i64 = 15_778_476;
const ADULT: i32 = 16;
/// Husband cohorts per wife cohort: category `c` is the cohort
/// `y + GAP_HI − c`, gap `c − GAP_HI` from 19 years younger to 30 older
/// (the pack's age-gap kernel's support).
const GAPS: usize = 50;
const GAP_HI: i32 = 19;
/// Union-age quantile cells of a wife cohort's interleave.
const CELLS: usize = 24;
/// Mothers' ages.
const A_LO: i32 = 15;
const A_HI: i32 = 45;
const AGES: usize = (A_HI - A_LO + 1) as usize;
/// The most of a cohort's adult men the husbands may take (expected).
const MEN_CAP: f64 = 0.95;
const GESTATION: i64 = 266 * 86_400;
/// Bits of an external id's birth year (counted from the world's first
/// cohort; worlds span at most 512 years).
const ID_YEAR_BITS: u32 = 9;

const T_SIZE: u64 = 1;
const T_SEX: u64 = 2;
const T_YOUNG: u64 = 3;
const T_WIVES: u64 = 4;
const T_TREE: u64 = 5;
const T_DEATH: u64 = 6;
const T_BLOCK: u64 = 7;
const T_BRIDGE: u64 = 8;
const T_PHASE: u64 = 9;
const T_SEP: u64 = 10;
const T_CELL: u64 = 11;
const T_GROUP: u64 = 12;
const T_OPEN: u64 = 13;
const T_POOL: u64 = 14;
const T_OPEN_MIX: u64 = 15;

/// A person: life cell, birth cohort and index in the cell cohort's life
/// order (women first; each sex: died young, then the partnered, then the
/// rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid {
    pub cell: u16,
    pub y: i32,
    pub i: u64,
}

/// A first union: the partners, its start, and how and when it ends (at
/// the first of separation, his death and hers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Union {
    pub wife: Pid,
    pub husband: Pid,
    pub start: i64,
    pub end: i64,
    /// 0 separation, 1 the husband's death, 2 the wife's.
    pub how: u8,
}

impl Union {
    /// The other partner.
    pub fn partner(&self, x: Pid) -> Pid {
        if x == self.wife { self.husband } else { self.wife }
    }
}

/// A death class: a rank interval of one cell cohort and sex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Dies before 16 (age-relative deaths).
    Young,
    /// Never partners: alive at 16.
    Never,
    /// Wives whose union age is in `[a0 + j, a0 + j + 1)`.
    Wife(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Class {
    pub kind: Kind,
    /// First life index and size.
    pub start: u64,
    pub n: u64,
    /// Requirement: alive at this time (absolute classes).
    pub req: Option<i64>,
}

/// One death class's constants, in one cache line: its life-index range,
/// its slot permutation `r ↦ (a·r + off) mod n` (with `a⁻¹`), its
/// birth-phase rotation `β` (women's classes) and `⌊(2⁶⁴ − 1)/n⌋` (the
/// phase's division by a multiply). No key hashing per call.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(64))]
struct ClassRec {
    start: u64,
    n: u64,
    a: u64,
    a_inv: u64,
    off: u64,
    beta: u64,
    recip: u64,
    /// Adult classes: the group's death time from the country table's age
    /// `a` as `mid + ⌊aft[0] + aft[1]·a⌋` seconds (accelerated failure time
    /// about the adult requirement; `[0, YEAR]` is the table itself,
    /// exactly).
    aft: [f32; 2],
}

/// A wife class's separation constants: its map's offset and its split's
/// systematic offset ([`SystematicShares::offset`]).
#[derive(Clone, Copy, Debug, Default)]
struct SepRec {
    b: u64,
    u: u64,
}

/// A cell cohort. Its first two cache lines hold what every lookup reads
/// (the sizes, the adults' requirement, where its class records start, the
/// wife-class guide); the rest is read rarely.
#[repr(C, align(64))]
struct Coh {
    n: u64,
    women: u64,
    young: [u64; 2],
    wives: u64,
    /// The middle of the birth year and the adults' requirement (alive at
    /// the end of the year they turn 16).
    mid: i64,
    adult_req: i64,
    /// The cohort's first class record ([`Cell::cls`]) and its wife classes.
    cls_at: u32,
    nwc: u32,
    /// For each `2^wshift` wives by rank `k` (`k >> wshift < 32`), the wife
    /// class of the first (at most the last class): a wife's class lies
    /// between two adjacent entries.
    wguide: [u8; 33],
    wshift: u8,
    /// The birth year's length in seconds (it starts at `mid − HALF_YEAR`).
    ylen: u32,
    /// The open wives: an exact Beatty set of `open_t` among the `wives`
    /// (offset `open_tau`); the others are in-group wives.
    open_t: u64,
    open_tau: u64,
    top: f64,
    median: f64,
    /// Wives with union age below `a0 + j`, `j = 0..`, ending at `wives`
    /// (union-age classes, for union times). Death classes are the same up
    /// to the last childbearing age; the rest are one class
    /// ([`Coh::db`]).
    mb: Vec<u64>,
    /// Husbands' segment starts by category `c` (wife cohort `y − GAP_HI +
    /// c`): in-group, then open (after all in-group ones).
    seg_in: [u64; GAPS + 1],
    seg_o: [u64; GAPS + 1],
}

impl Coh {
    /// Where wife death class `j` starts (`j ≤ nwc`; `nwc` ends at `wives`).
    #[inline]
    fn db(&self, j: usize) -> u64 {
        if j < self.nwc as usize { self.mb[j] } else { self.wives }
    }

    /// The wife death class of union-age class `j`.
    #[inline]
    fn death_class_of(&self, j: usize) -> usize {
        j.min(self.nwc as usize - 1)
    }

    /// The open-wives Beatty set.
    #[inline]
    fn open(&self) -> RationalBeatty {
        RationalBeatty { t: self.open_t, len: self.wives.max(1), tau: self.open_tau }
    }

    /// Adult men.
    fn men(&self) -> u64 {
        self.n - self.women - self.young[1]
    }
}

/// A year's birth-block starts, padded to a whole number of cache lines
/// (entries after the end are `u64::MAX`), with every eighth start in
/// [`Cell::block_top`]: a search reads one line of each.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct BlockRow([u64; 64]);

/// Eligible mothers of one (year, mother age), in one cache line: what a
/// mother lookup reads first. The row's alive prefix has the mother cohort's
/// wife classes plus one entries (one when that cohort doesn't exist); its
/// never-partnered alive are `elig[1] − (total − elig[0])`.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct EligRow {
    /// Eligible mothers by kind (married, single).
    elig: [u64; 2],
    /// Alive wives (the prefix's last entry).
    total: u64,
    /// Where the row's alive prefix starts in [`Cell::elig_alive`].
    off: u32,
    /// For each `g`, the class holding position `g << gshift` (positions
    /// below `total < 32 << gshift`), so the class of a position lies
    /// between two adjacent entries.
    guide: [u8; GUIDE + 1],
    gshift: u8,
}

impl EligRow {
    /// Never-partnered women alive through the year.
    #[inline]
    fn never_alive(&self) -> u64 {
        self.elig[1] - (self.total - self.elig[0])
    }
}

/// Buckets of an [`EligRow`]'s guide.
const GUIDE: usize = 32;

/// Every eligibility row's alive prefix, concatenated: 32-bit when every
/// entry fits (cohorts below 2³² wives: the `us` pack below about
/// ×70,000), else 64-bit.
enum Prefixes {
    Narrow(Vec<u32>),
    Wide(Vec<u64>),
}

impl Prefixes {
    fn new(v: Vec<u64>) -> Self {
        if v.iter().all(|&x| x <= u32::MAX as u64) {
            Prefixes::Narrow(v.into_iter().map(|x| x as u32).collect())
        } else {
            Prefixes::Wide(v)
        }
    }

    #[inline]
    fn get(&self, i: usize) -> u64 {
        match self {
            Prefixes::Narrow(v) => v[i] as u64,
            Prefixes::Wide(v) => v[i],
        }
    }

    /// `lo` plus the entries `off + lo + 1 ..= off + hi` at most `at`: the
    /// last `j` in `lo..=hi` whose entry is at most `at` (entries rise).
    #[inline]
    fn last_le(&self, off: usize, lo: usize, hi: usize, at: u64) -> usize {
        match self {
            Prefixes::Narrow(v) => lo + v[off + lo + 1..=off + hi].partition_point(|&p| p as u64 <= at),
            Prefixes::Wide(v) => lo + v[off + lo + 1..=off + hi].partition_point(|&p| p <= at),
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Prefixes::Narrow(v) => v.capacity() * 4,
            Prefixes::Wide(v) => v.capacity() * 8,
        }
    }
}

struct Elig {
    /// Wives alive through the year in classes `0..j`, for every `j ≤
    /// len − 1` (the married mothers are classes below `a`, the rest are
    /// single mothers: under option A nobody is held alive to their union).
    /// Inline (a cohort has at most [`MAX_WIFE_CLASSES`] wife death
    /// classes), so the build allocates nothing per row.
    alive: [u64; MAX_WIFE_CLASSES + 1],
    len: u8,
    /// Never-partnered women alive through the year.
    never_alive: u64,
}

impl Elig {
    fn alive(&self) -> &[u64] {
        &self.alive[..self.len as usize]
    }
}

/// The most wife death classes a cohort has: union-age years from `a0` to
/// the last childbearing age, then one (`merge_at + 1`, with `a0 ≥ 0`).
const MAX_WIFE_CLASSES: usize = (A_HI + 2) as usize;

/// A cell: one heritage group's people, every per-cohort and per-year
/// structure of the monotone world.
struct Cell {
    /// The cell's key (the world's key in a heritage-blind world).
    key: Key,
    /// The cell's heritage group (its index in the pack).
    group: usize,
    coh: Vec<Coh>,
    /// Every death class's constants (young women, young men,
    /// never-partnered women, adult men, then wife class `j` at `4 + j`),
    /// cohort by cohort from its `cls_at`.
    cls: Vec<ClassRec>,
    /// Wife classes' separation constants, in the same places.
    seps: Vec<SepRec>,
    /// Each cohort's bridge between birth order and life order (32 bytes
    /// each, so all of them stay in the first-level cache).
    bridges: Vec<CompactPerm4>,
}

/// A cell's identity (its tables are in the world's flat arrays, cell by
/// cell).
struct CellInfo {
    /// The cell's key (the world's key in a heritage-blind world).
    key: Key,
    /// The cell's heritage group (its index in the pack).
    group: usize,
}

/// A union space's tables: per wife cohort, its row over the space's
/// categories (cumulative, `cats + 1` entries) and the interleave laying the
/// space's wives of that cohort over them.
struct Space {
    cats: usize,
    rows: Vec<u64>,
    ilvs: Vec<InterleaveTable>,
}

impl Space {
    #[inline]
    fn row(&self, k: usize) -> &[u64] {
        &self.rows[k * (self.cats + 1)..(k + 1) * (self.cats + 1)]
    }
}

pub struct Mono<'a> {
    p: &'a Params,
    b_min: i32,
    y1: i32,
    a0: i32,
    /// Groups (cells); 1 in a heritage-blind world.
    groups: usize,
    cells: Vec<CellInfo>,
    /// Years from `b_min` (cohorts per cell) and from `y0` (birth years per
    /// cell).
    ny: usize,
    nb: usize,
    /// Cell cohorts, cell by cell (`cell · ny + y − b_min`).
    coh: Vec<Coh>,
    /// Every death class's constants (young women, young men,
    /// never-partnered women, adult men, then wife class `j` at `4 + j`),
    /// cohort by cohort from its `cls_at`.
    cls: Vec<ClassRec>,
    /// Wife classes' separation constants, in the same places.
    seps: Vec<SepRec>,
    /// Each cell cohort's bridge between birth order and life order (32
    /// bytes each), as `coh`.
    bridges: Vec<CompactPerm4>,
    /// Per cell and year from `y0` (`cell · nb + y − y0`): block starts in
    /// (kind, age) order, then the end; and every eighth start.
    blocks: Vec<BlockRow>,
    block_top: Vec<[u64; 8]>,
    /// Per cell, year from `y0` and mother age.
    elig_rows: Vec<EligRow>,
    /// Every row's alive prefix, concatenated.
    elig_alive: Prefixes,
    /// Union spaces: group `g`'s at `g`, then the open market (when there
    /// are groups).
    spaces: Vec<Space>,
    /// Death-age tables of the country's cohorts (every cell's: a group's
    /// adults scale them by its class records' `aft`): per cohort, adults'
    /// (women, men) of `adult_shape`, then the young's of `young_shape`.
    life: Vec<f32>,
    adult_shape: OctaveShape,
    young_shape: OctaveShape,
    /// Per wife cohort: the open market's wives as an exact interleave over
    /// groups (sizes: each group cohort's open wives).
    open_mix: Vec<InterleaveTable>,
    /// Per wife cohort: the open wives with a seat (the first in
    /// `open_mix` order); the rest found no man in reach (only in tiny
    /// worlds) and are void seats.
    open_seated: Vec<u64>,
    /// Per union year from `b_min + 1 + a0`: the dissolution classes'
    /// systematic shares, `[S₁, …, S₄₀, none]`.
    sep_shares: Vec<SystematicShares>,
    /// The first-union age law's shape `(q/(1 − q))^(1/shape)`, shared by
    /// every cohort (only the median varies): its lower tail over `q < ½`
    /// and its upper tail over `1 − q`, as quantile tables.
    union_q: [OctaveTable; 2],
    /// External ids: index bits (53 − 9 − the cell bits).
    id_index_bits: u32,
    /// Name tables, built on first use (`naming`).
    names: naming::NameCache,
    /// Residence: places, move tables and memos, built on first use.
    residence: std::sync::OnceLock<residence::Residence>,
    /// Schools and colleges, built on first use.
    institutions: std::sync::OnceLock<schools::Institutions>,
    /// Occupation tables, built on first use.
    occupations: std::sync::OnceLock<work::Occupations>,
    /// The world's people per person of the places' weights, by decade
    /// (work sizes establishments by it).
    sample_share: std::sync::OnceLock<Vec<f64>>,
}

/// A keyed systematic split of `n` by weight (exact total, each part the
/// floor or ceiling of its share), then each part over its cap passes the
/// excess to the next parts with room, cyclically. O(parts).
fn capped_split(n: u64, w: &[f64], cap: &[u64], key: Key, out: &mut [u64]) {
    let len = w.len();
    let ww: Vec<f64> = (0..len).map(|i| if cap[i] > 0 { w[i].max(0.0) } else { 0.0 }).collect();
    if ww.iter().all(|&x| x <= 0.0) {
        procedural_core::partition::apportion_systematic(n, &cap.iter().map(|&c| c as f64).collect::<Vec<_>>(), key, out);
    } else {
        procedural_core::partition::apportion_systematic(n, &ww, key, out);
    }
    let mut excess = 0u64;
    for i in 0..len {
        if out[i] > cap[i] {
            excess += out[i] - cap[i];
            out[i] = cap[i];
        }
    }
    let mut i = 0;
    while excess > 0 {
        let give = (cap[i] - out[i]).min(excess);
        out[i] += give;
        excess -= give;
        i = (i + 1) % len;
    }
}

/// The kind whose mothers a block of kind `kind` goes to, given the
/// eligible mothers by kind: its own, else the other, else none.
#[inline]
fn effective_kind(elig: &[u64; 2], kind: usize) -> Option<usize> {
    if elig[kind] > 0 {
        Some(kind)
    } else if elig[1 - kind] > 0 {
        Some(1 - kind)
    } else {
        None
    }
}

/// Returns the build's freed scratch memory to the system (glibc keeps it
/// in per-thread arenas otherwise); elsewhere a no-op.
fn release_free_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: glibc's malloc_trim only releases free memory.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// A hint to start loading `x`'s cache line (no effect on results).
#[inline(always)]
fn prefetch<T>(x: &T) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch never faults and reads nothing into the program.
    unsafe {
        std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(x as *const T as *const i8)
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = x;
}

/// Seconds after the middle of the birth year of a death at table age `a`
/// under a class's scaling (`[0, YEAR]`: exactly `⌊a·YEAR⌋`).
#[inline(always)]
fn aft_secs(aft: [f32; 2], a: f64) -> i64 {
    (aft[0] as f64 + aft[1] as f64 * a) as i64
}

/// A class's slot permutation from its record.
#[inline]
fn rec_perm(r: &ClassRec) -> AffinePerm {
    AffinePerm::from_raw(r.n, r.a, r.a_inv, r.off)
}

/// `rec_perm(r).fwd(x)`, reducing by the stored reciprocal when the
/// product fits 64 bits (the same value).
#[inline]
fn rec_fwd(r: &ClassRec, x: u64) -> u64 {
    debug_assert!(x < r.n);
    match r.a.checked_mul(x).and_then(|p| p.checked_add(r.off)) {
        Some(p) => procedural_core::perm::rem_by_inverse(p, r.n, r.recip),
        None => rec_perm(r).fwd(x),
    }
}

/// `rec_perm(r).inv(v)`, likewise.
#[inline]
fn rec_inv(r: &ClassRec, v: u64) -> u64 {
    debug_assert!(v < r.n);
    let d = if v >= r.off { v - r.off } else { v + r.n - r.off };
    match r.a_inv.checked_mul(d) {
        Some(p) => procedural_core::perm::rem_by_inverse(p, r.n, r.recip),
        None => rec_perm(r).inv(v),
    }
}

/// A class's record index within its cohort: young (women, men),
/// never-partnered women, adult men, then wife class `j` at `4 + j`.
#[inline]
fn class_index(kind: Kind, female: bool) -> usize {
    match kind {
        Kind::Young => (!female) as usize,
        Kind::Never => 2 + (!female) as usize,
        Kind::Wife(j) => 4 + j,
    }
}

/// The phase every class's births of year `y` turn by, so a mother's
/// children have different days of the year: back `ε = 0.0937` of a year
/// per year. A mother's births in consecutive years are then `1 − ε` years
/// apart (at least 330 days), or `2 − ε` when her phase wraps, never closer; her
/// birthdays cycle about every 10.7 years. Within a year it is one rotation
/// for every class, so "born by t" stays at most two slot runs per class.
#[inline]
fn year_turn(y: i32) -> u64 {
    // Q − round(0.0937·Q), odd.
    (y as i64 as u64).wrapping_mul(Q - 402_438_437) & (Q - 1)
}

/// A mother's phase in her child's birth year `y`, from her class record
/// and slot: as [`Mono::phase`], turned by [`year_turn`].
#[inline]
fn rec_phase(r: &ClassRec, rho: u64, y: i32) -> u64 {
    // ⌊(2ρ + 1)·Q/2n⌋ = ⌊(2ρ + 1)·2³¹/n⌋: by the stored reciprocal (at
    // most one short, then corrected) while the dividend fits 64 bits.
    let h = if rho < 1 << 32 {
        let x = (2 * rho + 1) << 31;
        let mut q = ((x as u128 * r.recip as u128) >> 64) as u64;
        if x - q * r.n >= r.n {
            q += 1;
        }
        q
    } else {
        mul_div(2 * rho + 1, Q, 2 * r.n)
    };
    (h + r.beta + year_turn(y)) % Q
}

/// Fixed-point unit of birth phases.
const Q: u64 = 1 << 32;

/// Slots `ρ ∈ [lo, n)` of a class whose phase `(⌊(2ρ+1)Q/2n⌋ + β) mod Q`
/// is at most `p`: at most two intervals.
fn phase_runs(n: u64, beta: u64, p: u64, lo: u64) -> [std::ops::Range<u64>; 2] {
    // Slots with h(ρ) ≤ x: ⌈2n(x+1)/Q⌉ / 2 (x < Q).
    let upto = |x: i128| -> u64 {
        if x < 0 {
            0
        } else if x >= Q as i128 - 1 {
            n
        } else {
            (((2 * n as u128 * (x as u128 + 1)).div_ceil(Q as u128)) / 2).min(n as u128) as u64
        }
    };
    let (b, p) = (beta as i128, p as i128);
    // h + β < Q and h + β ≤ p: h ≤ min(p − β, Q − 1 − β).
    let a = 0..upto((p - b).min(Q as i128 - 1 - b));
    // h + β ≥ Q and h + β − Q ≤ p: Q − β ≤ h ≤ Q − β + p.
    let c = upto(Q as i128 - 1 - b)..upto(Q as i128 - b + p);
    let clip = |r: std::ops::Range<u64>| r.start.max(lo)..r.end.max(lo);
    [clip(a), clip(c)]
}

/// `a·b mod n`, in 64 bits when the product fits.
#[inline]
fn mul_mod(a: u64, b: u64, n: u64) -> u64 {
    match a.checked_mul(b) {
        Some(p) => p % n,
        None => (a as u128 * b as u128 % n as u128) as u64,
    }
}

fn qrc(n: u64, f: f64) -> u64 {
    ((n as f64 * f - 0.5).ceil().max(0.0) as u64).min(n)
}

fn floor_u(x: f64) -> u64 {
    x.floor().max(0.0) as u64
}

/// Mothers' ages at birth, married (`kind` 0) or single (1): the pack's
/// algebraic sigmoid CDF over the fertile window.
fn mother_age_cdf(p: &Params, y: i32, kind: usize, a: f64) -> f64 {
    let m = &p.fertility.mother_age;
    let mu = m.peak.at(y) + if kind == 1 { m.single_offset } else { 0.0 };
    truncated_sigmoid_cdf(a, mu, m.scale, A_LO as f64, (A_HI + 1) as f64)
}

/// A cohort's Siler law by sex: infant terms from its birth year,
/// background from 30 years on, old age from 60, each scaled by the group's
/// factors in those years (`None`: the base law, untouched).
fn cohort_siler(p: &Params, group: Option<Heritage>, sex: Sex, y: i32) -> Siler {
    let m = &p.mortality;
    let (i, b, o) = (m.siler(sex, y), m.siler(sex, y + 30), m.siler(sex, y + 60));
    match group {
        None => Siler { infant: i.infant, decay: i.decay, background: b.background, old: o.old, slope: o.slope },
        Some(h) => {
            let f = |yy: i32| p.heritage.mortality_factor(h, yy);
            Siler {
                infant: i.infant * f(y).infant,
                decay: i.decay,
                background: b.background * f(y + 30).adult,
                old: o.old * f(y + 60).adult,
                slope: o.slope,
            }
        }
    }
}

/// Bits needed for values below `n` (0 for `n ≤ 1`).
fn bits_for(n: usize) -> u32 {
    if n <= 1 { 0 } else { usize::BITS - (n - 1).leading_zeros() }
}

/// What pass 1 settles per cell cohort: its size and women, and its laws.
#[derive(Clone, Copy)]
struct CohortSize {
    n: u64,
    women: u64,
    siler: [Siler; 2],
    /// The men's law of the cohort two years older (the husbands').
    husband_siler: Siler,
}

impl<'a> Mono<'a> {
    /// The world at `mult` times the pack's population, with one cell per
    /// heritage group of the pack.
    pub fn new(p: &'a Params, seed: u64, mult: f64) -> Self {
        Self::build(p, seed, mult, false)
    }

    /// The heritage-blind world: everyone in one cell, the base laws.
    pub fn blind(p: &'a Params, seed: u64, mult: f64) -> Self {
        Self::build(p, seed, mult, true)
    }

    fn build(p: &'a Params, seed: u64, mult: f64, blind: bool) -> Self {
        let key = Key::from_seed(seed).with(0x3030);
        let b_min = p.y0 - p.founder_max_age;
        let y1 = p.y1;
        let fu = &p.unions.first_union;
        let a0 = fu.origin.floor() as i32;
        // Union-age classes from here on are one wife death class: union
        // ages past the last childbearing age.
        let merge_at = (A_HI + 1 - a0).max(1) as usize;
        let births = &p.fertility.births;
        let scale = p.founder_births / (births.at(p.y0 - 1) * 1e6) * mult;
        let msb = p.male_share_at_birth;
        let groups = if blind { 1 } else { p.heritage_count().max(1) };
        let her = |g: usize| if blind { None } else { Some(Heritage(g as u8)) };
        let cell_key = |g: usize| if blind { key } else { key.with2(T_CELL, g as u64) };
        let years: Vec<i32> = (b_min..=y1).collect();
        let at = |y: i32| (y - b_min) as usize;
        let ok = |s: i32| s >= b_min && s <= y1;

        let t0 = std::time::Instant::now();
        let lap = |what: &str| if std::env::var("MONO_BUILD_TIMES").is_ok() { eprintln!("build {what}: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3) };
        // 1–2. Cell cohorts, in waves of 15 years (the youngest mothers'
        // age): each year's births split over groups by their eligible
        // mothers (realized: women alive through the year, all in cohorts of
        // earlier waves) times the group's fertility, never more than a
        // group's eligible mothers; then each cohort's young, wives and
        // classes, the open split and its death tables, in parallel within
        // the wave.
        let totals: Vec<u64> = years
            .iter()
            .map(|&y| {
                let e = if y < p.y0 {
                    p.founder_births * mult * procedural_core::dmath::exp(p.founder_growth * (y - (p.y0 - 1)) as f64)
                } else {
                    births.at(y) * 1e6 * scale
                };
                floor_u(e + key.with2(T_SIZE, y as u64).unit())
            })
            .collect();
        // Death ages by cohort and sex, as quantile tables of one shape (deep
        // enough for the largest cohort: a cell cohort is at most its year's
        // births), in one arena per cell: adults' (years from the middle of
        // the birth year, given alive at the adult requirement), then the
        // young's (age, given death before 16).
        let deepest = OctaveTable::octaves_for(totals.iter().copied().max().unwrap_or(1));
        let (adult_shape, young_shape) = (OctaveShape::new(0, deepest), OctaveShape::new(0, deepest));
        let (la, ly) = (adult_shape.len(), young_shape.len());
        let stride = 2 * (la + ly);
        let mut cells: Vec<Cell> = (0..groups)
            .map(|g| Cell {
                key: cell_key(g),
                group: g,
                coh: Vec::with_capacity(years.len()),
                cls: Vec::new(),
                seps: Vec::new(),
                bridges: Vec::with_capacity(years.len()),
            })
            .collect();
        let mut life = vec![0f32; years.len() * stride];
        let mut sizes: Vec<Vec<CohortSize>> = vec![Vec::with_capacity(years.len()); groups];
        let mut pools_o: Vec<Vec<u64>> = vec![Vec::with_capacity(years.len()); groups];
        // Per cell, (year from y0, mother age): the eligible mothers.
        // Per cell: its eligibility rows (year from y0, mother age) and their
        // alive prefixes, concatenated (offsets per cell).
        let nrows = (y1 - p.y0 + 1).max(0) as usize * AGES;
        let mut erows: Vec<Vec<EligRow>> = (0..groups).map(|_| Vec::with_capacity(nrows)).collect();
        let mut ealive: Vec<Vec<u64>> = vec![Vec::new(); groups];
        // A row from its eligible mothers (`mc`: the married classes, wives
        // with union age below the mother age).
        let make_row = |e: &Elig, mc: usize, off: u32| -> EligRow {
            let al = e.alive();
            let last = al.len() - 1;
            let elig = [al[mc], al[last] - al[mc] + e.never_alive];
            let total = al[last];
            let gshift = (64 - total.leading_zeros()).saturating_sub(5) as u8;
            let guide = std::array::from_fn(|g| {
                let pos = (g as u64) << gshift;
                (al.partition_point(|&v| v <= pos).max(1) - 1) as u8
            });
            EligRow { elig, total, off, guide, gshift }
        };
        // Eligible mothers of (year y, age a) in a cell: its cohort `y − a`'s
        // wives alive through the year, by class (a prefix), and its
        // never-partnered women alive through it.
        let elig_of = |cell: &Cell, life: &[f32], y: i32, a: i32| -> Elig {
            let ym = y - a;
            if !ok(ym) {
                return Elig { alive: [0; MAX_WIFE_CLASSES + 1], len: 1, never_alive: 0 };
            }
            let c = &cell.coh[at(ym)];
            let t = year_start(y + 1);
            // One threshold for every adult class of the cohort.
            let th = if t <= c.adult_req {
                None
            } else {
                let base = at(ym) * stride;
                let aft = cell.cls[c.cls_at as usize + class_index(Kind::Never, true)].aft;
                let v = ((t - c.mid + 1) as f64 - aft[0] as f64) / aft[1] as f64;
                adult_shape.threshold_near(&life[base..base + la], v, |a| (c.mid + aft_secs(aft, a)).max(c.adult_req + 1) <= t)
            };
            let dead = |n: u64| th.map_or(0, |w| count_from_threshold(n, w));
            let mut alive = [0u64; MAX_WIFE_CLASSES + 1];
            let nwc = c.nwc as usize;
            for j in 0..nwc {
                let n = c.db(j + 1) - c.db(j);
                alive[j + 1] = alive[j] + n - dead(n);
            }
            let never = c.women - c.young[0] - c.wives;
            Elig { alive, len: (nwc + 1) as u8, never_alive: never - dead(never) }
        };
        // A country cohort's death tables (the pack's law, no group
        // factors), into `out`.
        // Remaining life at the adult requirement, for the groups' scaling
        // (a yearly table: only ratios of two such values are used).
        let remaining = |s: &Siler, from: f64| HazardTable::from_siler(s, 130.0, 1.0).remaining_life(from);
        let tables = |y: i32, out: &mut [f32]| -> [f64; 2] {
            let laws = [Sex::Female, Sex::Male].map(|s| cohort_siler(p, None, s, y));
            let life = laws.map(|s| HazardTable::from_siler(&s, 130.0, 0.25));
            let mid = year_start(y) + HALF_YEAR;
            let adult_req = year_start(y + 1) + (ADULT as f64 * YEAR) as i64;
            let from = (adult_req - mid) as f64 / YEAR;
            let (adults, young) = out.split_at_mut(2 * la);
            for (t, o) in life.iter().zip(adults.chunks_mut(la)) {
                let h0 = t.cumulative(from);
                adult_shape.tabulate_into(|w| t.age_at(h0 - procedural_core::dmath::ln(w)).max(from), o);
            }
            for (t, o) in life.iter().zip(young.chunks_mut(ly)) {
                let span = 1.0 - procedural_core::dmath::exp(-t.cumulative(ADULT as f64));
                young_shape.tabulate_into(|w| t.age_at(-procedural_core::dmath::ln(1.0 - (1.0 - w) * span)).clamp(0.0, ADULT as f64), o);
            }
            if groups > 1 { laws.map(|s| remaining(&s, from)) } else { [1.0; 2] }
        };
        // A cell cohort's young, wives and their classes, and the open split
        // (`base_e`: the country law's remaining life at the adult
        // requirement, by sex).
        let pass2 = |g: usize, y: i32, cs: CohortSize, base_e: [f64; 2]| -> (Coh, Vec<(ClassRec, SepRec)>, CompactPerm4, u64) {
            let ck = cell_key(g);
            let CohortSize { n, women, siler, husband_siler: mh } = cs;
            let young: [u64; 2] = std::array::from_fn(|s| {
                let ns = if s == 0 { women } else { n - women };
                let q = 1.0 - siler[s].survival(ADULT as f64);
                floor_u(ns as f64 * q + ck.with3(T_YOUNG, y as u64, s as u64).unit()).min(ns)
            });
            let median = fu.median_female.at(y + 25);
            let top = log_logistic_cdf((y1 - y) as f64, fu.origin, median, fu.shape);
            // Survivors' convention (`thinking/claude/010`): the pack's
            // share is of women alive at ~50. Under option A a union is
            // void if the husband (about two years older) is dead at its
            // start; the survivor's lost union is compensated, nothing
            // else is.
            let ph = mh.survival(median + 2.0) / mh.survival(ADULT as f64);
            let share = (fu.ever_partnered.at(y + 25) / ph.max(0.5)).min(1.0) * top;
            let adults = women - young[0];
            let wives = floor_u(adults as f64 * share + ck.with2(T_WIVES, y as u64).unit()).min(adults);
            let mut mb = vec![0u64];
            if wives > 0 && top > 0.0 {
                let mut a = a0 + 1;
                loop {
                    let f = (log_logistic_cdf(a as f64, fu.origin, median, fu.shape) / top).min(1.0);
                    let c = qrc(wives, f).max(*mb.last().unwrap());
                    if c >= wives || a > 130 {
                        mb.push(wives);
                        break;
                    }
                    mb.push(c);
                    a += 1;
                }
            } else {
                mb.push(wives);
            }
            // Wife death classes: one per union-age year to the last
            // childbearing age (married mothers are a prefix of them), then
            // one for every later union.
            let nwc = (mb.len() - 1).min(merge_at + 1);
            // Mortality tabulated once (cumulative hazard every quarter
            // year), then its two conditional laws as quantile tables.
            // The group's adult ages: the country table's, scaled about the
            // adult requirement so that the remaining life there is the
            // group law's (accelerated failure time; the table itself with
            // no group).
            let from = (year_start(y + 1) + (ADULT as f64 * YEAR) as i64 - (year_start(y) + HALF_YEAR)) as f64 / YEAR;
            let aft: [[f32; 2]; 2] = std::array::from_fn(|s| match her(g) {
                None => [0.0, YEAR as f32],
                Some(_) => {
                    let k = remaining(&siler[s], from) / base_e[s];
                    [(from * (1.0 - k) * YEAR) as f32, (k * YEAR) as f32]
                }
            });
            let men_adult = n - women - young[1];
            let bridge = CompactPerm4::new(n, ck.with2(T_BRIDGE, y as u64));
            // Offsets and rotations, keyed by class (`ckey`); (female, key code, start, size, wife class).
            let ckey = |female: bool, code: u64| ck.with3(T_DEATH, ((y as u64) << 1) | female as u64, code);
            let classes: Vec<(bool, u64, u64, u64, Option<usize>)> = [
                (true, 0u64, 0, young[0], None),
                (false, 0, women, young[1], None),
                (true, 1, young[0] + wives, women - young[0] - wives, None),
                (false, 1, women + young[1], men_adult, None),
            ]
            .into_iter()
            .chain((0..nwc).map(|j| {
                let (s, e) = (mb[j], if j + 1 < nwc { mb[j + 1] } else { wives });
                (true, 2 + (j as u64) * 4, young[0] + s, e - s, Some(j))
            }))
            .collect();
            let recs: Vec<(ClassRec, SepRec)> = classes
                .iter()
                .map(|&(f, code, start, size, wife)| {
                    let (a, a_inv) = procedural_core::perm::golden_pair(size);
                    let sep = match wife {
                        Some(j) => {
                            let k = ck.with3(T_SEP, y as u64, j as u64);
                            SepRec { b: k.below(size.max(1)), u: SystematicShares::offset(k.with(1)) }
                        }
                        None => SepRec::default(),
                    };
                    let rec = ClassRec {
                        start,
                        n: size,
                        a,
                        a_inv,
                        off: if size == 0 { 0 } else { ckey(f, code).below(size) },
                        beta: ckey(f, code).with(T_PHASE).below(Q),
                        recip: u64::MAX.checked_div(size).unwrap_or(0),
                        aft: if code == 0 { [0.0, YEAR as f32] } else { aft[(!f) as usize] },
                    };
                    (rec, sep)
                })
                .collect();
            let mid = year_start(y) + HALF_YEAR;
            let adult_req = year_start(y + 1) + (ADULT as f64 * YEAR) as i64;
            // The wife-class guide: buckets of 2^wshift ranks, fewer than 32.
            assert!(nwc <= MAX_WIFE_CLASSES, "{y}: {nwc} wife classes");
            let wshift = (64 - wives.leading_zeros()).saturating_sub(5) as u8;
            let wguide = std::array::from_fn(|g| {
                let k = (g as u64) << wshift;
                (mb[..nwc].partition_point(|&b| b <= k).max(1) - 1).min(nwc.max(1) - 1) as u8
            });
            let ylen = (year_start(y + 1) - year_start(y)) as u32;
            debug_assert_eq!(mid - HALF_YEAR, year_start(y));
            // The open market's share of the wives and of the men.
            let (open_t, open_tau, pool_o) = match her(g) {
                None => (0, 0, 0),
                Some(h) => {
                    let of = p.heritage.open_market_share(y + 25, h, Sex::Female) * p.open_market_boost;
                    let om = p.heritage.open_market_share(y + 27, h, Sex::Male) * p.open_market_boost;
                    let t = floor_u(of.min(1.0) * wives as f64 + ck.with2(T_OPEN, y as u64).unit()).min(wives);
                    let tau = if wives == 0 { 0 } else { ck.with2(T_OPEN, y as u64).with(1).below(wives) };
                    let pool = floor_u(om.min(1.0) * men_adult as f64 + ck.with2(T_POOL, y as u64).unit()).min(men_adult);
                    (t, tau, pool)
                }
            };
            let coh = Coh {
                n,
                women,
                young,
                wives,
                mid,
                adult_req,
                cls_at: 0,
                nwc: nwc as u32,
                wguide,
                wshift,
                ylen,
                open_t,
                open_tau,
                top,
                median,
                mb,
                seg_in: [0; GAPS + 1],
                seg_o: [0; GAPS + 1],
            };
            (coh, recs, bridge, pool_o)
        };
        const _: () = assert!(A_LO >= 1, "waves need mothers older than the wave");
        let mut y_next = b_min;
        while y_next <= y1 {
            let wave: Vec<i32> = (y_next..=(y_next + A_LO - 1).min(y1)).collect();
            // The wave's eligible mothers (from y0 on: founders have none).
            let need: Vec<(usize, i32, i32)> = (0..groups)
                .flat_map(|g| wave.iter().filter(|&&y| y >= p.y0).flat_map(move |&y| (A_LO..=A_HI).map(move |a| (g, y, a))))
                .collect();
            let got: Vec<Elig> = need.par_iter().map(|&(g, y, a)| elig_of(&cells[g], &life, y, a)).collect();
            for (&(g, y, a), e) in need.iter().zip(got) {
                let mc = if ok(y - a) { ((a - a0).max(0) as usize).min(cells[g].coh[at(y - a)].nwc as usize) } else { 0 };
                let row = make_row(&e, mc, ealive[g].len() as u32);
                debug_assert_eq!(row.never_alive(), e.never_alive);
                erows[g].push(row);
                ealive[g].extend_from_slice(e.alive());
            }

            for &y in &wave {
                let n = totals[at(y)];
                let mut parts = vec![0u64; groups];
                if groups == 1 {
                    parts[0] = n;
                } else if y < p.y0 {
                    let w = p.floored(&p.heritage.founder_mix(p.y0));
                    if w.iter().any(|&x| x > 0.0) {
                        procedural_core::partition::apportion_systematic(n, &w, key.with2(T_GROUP, y as u64), &mut parts);
                    } else {
                        parts[0] = n;
                    }
                } else {
                    let k = (y - p.y0) as usize * AGES;
                    let total = |e: &EligRow| e.elig[0] + e.elig[1];
                    let room: Vec<u64> = (0..groups).map(|g| erows[g][k..k + AGES].iter().map(total).sum()).collect();
                    let w: Vec<f64> = (0..groups)
                        .map(|g| {
                            let mothers: f64 = (A_LO..=A_HI)
                                .map(|a| {
                                    let shape = mother_age_cdf(p, y, 0, a as f64 + 1.0) - mother_age_cdf(p, y, 0, a as f64);
                                    total(&erows[g][k + (a - A_LO) as usize]) as f64 * shape
                                })
                                .sum();
                            mothers * p.heritage.fertility_factor(Heritage(g as u8), y)
                        })
                        .collect();
                    let r: u64 = room.iter().sum();
                    assert!(r >= n, "{y}: {n} births, {r} eligible mothers");
                    capped_split(n, &w, &room, key.with2(T_GROUP, y as u64), &mut parts);
                }
                for g in 0..groups {
                    let ng = parts[g];
                    let women = floor_u(ng as f64 * (1.0 - msb) + cell_key(g).with2(T_SEX, y as u64).unit()).min(ng);
                    let siler = [Sex::Female, Sex::Male].map(|s| cohort_siler(p, her(g), s, y));
                    let husband_siler = cohort_siler(p, her(g), Sex::Male, y - 2);
                    sizes[g].push(CohortSize { n: ng, women, siler, husband_siler });
                }
            }
            let jobs: Vec<(usize, i32)> = (0..groups).flat_map(|g| wave.iter().map(move |&y| (g, y))).collect();
            let span = at(wave[0]) * stride..(at(*wave.last().unwrap()) + 1) * stride;
            let base_e: Vec<[f64; 2]> = life[span].par_chunks_mut(stride).zip(wave.par_iter()).map(|(out, &y)| tables(y, out)).collect();
            let built: Vec<(Coh, Vec<(ClassRec, SepRec)>, CompactPerm4, u64)> =
                jobs.par_iter().map(|&(g, y)| pass2(g, y, sizes[g][at(y)], base_e[(y - wave[0]) as usize])).collect();

            for ((g, _), (mut c, r, bridge, pool_o)) in jobs.iter().copied().zip(built) {
                let cell = &mut cells[g];
                cell.bridges.push(bridge);
                c.cls_at = cell.cls.len() as u32;
                cell.cls.extend(r.iter().map(|p| p.0));
                cell.seps.extend(r.iter().map(|p| p.1));
                cell.coh.push(c);
                pools_o[g].push(pool_o);
            }
            y_next += A_LO;
        }
        drop(sizes);


        lap("cohorts");
        // 3. Unions. The pack's kernel: shift-invariant, by the gap alone, so
        // every union-age cell of a wife cohort has the same shares. Each
        // cell's demand is still added once per cell (the same float sums).
        let gw: [f64; GAPS] = std::array::from_fn(|c| p.unions.age_gap_weight(c as i32 - GAP_HI));
        let mut spaces: Vec<Space> = Vec::with_capacity(groups + 1);
        // Per group and cohort: the men not married in the group's space
        // (the open market's caps).
        let mut left_in: Vec<Vec<u64>> = Vec::with_capacity(groups);
        // 3a. Each group's own space: its in-group wives with its in-group
        // men, availability factors (a few passes) keeping each husband
        // cohort's expected demand within its in-group men. A wife cohort
        // with fewer in-group men in reach than wives (a small group) sends
        // the rest to the open market: small groups marry out more. Groups
        // in parallel.
        let per_group: Vec<(Space, Vec<(usize, u64)>, Vec<u64>)> = (0..groups).into_par_iter().map(|g| {
            let coh = &cells[g].coh;
            let pool = |k: usize| coh[k].men() - pools_o[g][k];
            let wives_in = |k: usize| coh[k].wives - coh[k].open_t;
            let mut f = vec![1.0f64; coh.len()];
            let shares_of = |yw: i32, f: &[f64]| -> [f64; GAPS] {
                std::array::from_fn(|c| {
                    let s = yw + GAP_HI - c as i32;
                    if ok(s) { f[at(s)] * gw[c] } else { 0.0 }
                })
            };
            for _ in 0..12 {
                let mut demand = vec![0.0f64; coh.len()];
                for &yw in &years {
                    let w = wives_in(at(yw)) as f64;
                    if w == 0.0 {
                        continue;
                    }
                    let sh = shares_of(yw, &f);
                    let tot: f64 = sh.iter().sum();
                    let per_cell = w / CELLS as f64;
                    for c in 0..GAPS {
                        if sh[c] > 0.0 {
                            let v = per_cell * sh[c] / tot;
                            let d = &mut demand[at(yw + GAP_HI - c as i32)];
                            for _ in 0..CELLS {
                                *d += v;
                            }
                        }
                    }
                }
                for k in 0..coh.len() {
                    let cap = MEN_CAP * pool(k) as f64;
                    if demand[k] > cap {
                        f[k] *= cap / demand[k];
                    }
                }
            }
            // The integer table, in year order: each wife cohort's wives
            // split over husband cohorts by the kernel (with the factors),
            // capped by the husbands each cohort has left.
            let mut left: Vec<u64> = (0..coh.len()).map(pool).collect();
            let mut rows = vec![0u64; coh.len() * (GAPS + 1)];
            let mut extra_open: Vec<(usize, u64)> = Vec::new();
            let ck = cells[g].key;
            for &yw in &years {
                let w = wives_in(at(yw));
                if w == 0 {
                    continue;
                }
                let sh = shares_of(yw, &f);
                let weight = |c: usize| (0..CELLS).map(|_| sh[c]).sum::<f64>();
                let cap = |c: usize| {
                    let s = yw + GAP_HI - c as i32;
                    if ok(s) { left[at(s)] } else { 0 }
                };
                let room: u64 = (0..GAPS).map(cap).sum();
                let take = w.min(room);
                if take < w {
                    assert!(groups > 1, "{yw}: {w} wives, room for {room}");
                    extra_open.push((at(yw), w - take));
                }
                let mut row = [0u64; GAPS];
                capped_split(take, &(0..GAPS).map(weight).collect::<Vec<_>>(), &(0..GAPS).map(cap).collect::<Vec<_>>(), ck.with2(T_TREE, yw as u64).with(1), &mut row);
                let r = &mut rows[at(yw) * (GAPS + 1)..(at(yw) + 1) * (GAPS + 1)];
                for c in 0..GAPS {
                    if row[c] > 0 {
                        left[at(yw + GAP_HI - c as i32)] -= row[c];
                    }
                    r[c + 1] = r[c] + row[c];
                }
            }
            let ilvs = years
                .iter()
                .map(|&yw| {
                    let r = &rows[at(yw) * (GAPS + 1)..(at(yw) + 1) * (GAPS + 1)];
                    let k = ck.with2(T_TREE, yw as u64);
                    InterleaveTable::new(GAPS, |i: usize| r[i], |node: u64| k.with(node).below(u64::MAX))
                })
                .collect();
            let men_left: Vec<u64> = (0..coh.len()).map(|k| left[k] + pools_o[g][k]).collect();
            (Space { cats: GAPS, rows, ilvs }, extra_open, men_left)
        }).collect();
        for (g, (space, extra_open, men_left)) in per_group.into_iter().enumerate() {
            spaces.push(space);
            for (k, extra) in extra_open {
                cells[g].coh[k].open_t += extra;
            }
            left_in.push(men_left);
        }
        lap("group spaces");
        // 3b. The open market (with groups): every group's open wives with
        // every group's open men. Categories are (husband cohort c, group h)
        // at `c·G + h`, weighted by the kernel and each group's open pool.
        let mut open_mix = Vec::new();
        let mut open_seated = Vec::new();
        if groups > 1 {
            let cats = GAPS * groups;
            let n = years.len();
            let wives_o = |k: usize| -> u64 { (0..groups).map(|g| cells[g].coh[k].open_t).sum() };
            let pool_sum: Vec<f64> = (0..n).map(|k| (0..groups).map(|h| pools_o[h][k] as f64).sum()).collect();
            let mut f = vec![vec![1.0f64; n]; groups];
            let weights = |yw: i32, f: &[Vec<f64>]| -> Vec<f64> {
                let mut w = vec![0.0f64; cats];
                for c in 0..GAPS {
                    let s = yw + GAP_HI - c as i32;
                    if !ok(s) || pool_sum[at(s)] == 0.0 {
                        continue;
                    }
                    for h in 0..groups {
                        w[c * groups + h] = f[h][at(s)] * gw[c] * pools_o[h][at(s)] as f64 / pool_sum[at(s)];
                    }
                }
                w
            };
            for _ in 0..12 {
                let mut demand = vec![vec![0.0f64; n]; groups];
                for &yw in &years {
                    let w = wives_o(at(yw)) as f64;
                    if w == 0.0 {
                        continue;
                    }
                    let sh = weights(yw, &f);
                    let tot: f64 = sh.iter().sum();
                    if tot == 0.0 {
                        continue;
                    }
                    for (cat, &v) in sh.iter().enumerate() {
                        if v > 0.0 {
                            demand[cat % groups][at(yw + GAP_HI - (cat / groups) as i32)] += w * v / tot;
                        }
                    }
                }
                for h in 0..groups {
                    for k in 0..n {
                        let cap = MEN_CAP * pools_o[h][k] as f64;
                        if demand[h][k] > cap {
                            f[h][k] *= cap / demand[h][k];
                        }
                    }
                }
            }
            // Caps: every man not married in his group's space (weights
            // keep the open pools' composition).
            let mut left: Vec<Vec<u64>> = left_in;
            let mut rows = vec![0u64; n * (cats + 1)];
            open_seated = vec![0u64; n];
            for &yw in &years {
                let w = wives_o(at(yw));
                if w == 0 {
                    continue;
                }
                let sh = weights(yw, &f);
                let cap: Vec<u64> = (0..cats)
                    .map(|cat| {
                        let s = yw + GAP_HI - (cat / groups) as i32;
                        if ok(s) { left[cat % groups][at(s)] } else { 0 }
                    })
                    .collect();
                // In a tiny world the men in reach may run out: the last open
                // wives (in `open_mix` order) are then void seats.
                let take = w.min(cap.iter().sum());
                open_seated[at(yw)] = take;
                let mut row = vec![0u64; cats];
                if take > 0 {
                    capped_split(take, &sh, &cap, key.with2(T_OPEN, yw as u64).with(1), &mut row);
                }
                let r = &mut rows[at(yw) * (cats + 1)..(at(yw) + 1) * (cats + 1)];
                for cat in 0..cats {
                    if row[cat] > 0 {
                        left[cat % groups][at(yw + GAP_HI - (cat / groups) as i32)] -= row[cat];
                    }
                    r[cat + 1] = r[cat] + row[cat];
                }
            }
            let ilvs = years
                .iter()
                .map(|&yw| {
                    let r = &rows[at(yw) * (cats + 1)..(at(yw) + 1) * (cats + 1)];
                    let k = key.with2(T_OPEN, yw as u64).with(2);
                    InterleaveTable::new(cats, |i: usize| r[i], |node: u64| k.with(node).below(u64::MAX))
                })
                .collect();
            spaces.push(Space { cats, rows, ilvs });
            open_mix = years
                .iter()
                .map(|&yw| {
                    let mut pre = vec![0u64; groups + 1];
                    for g in 0..groups {
                        pre[g + 1] = pre[g] + cells[g].coh[at(yw)].open_t;
                    }
                    let k = key.with2(T_OPEN_MIX, yw as u64);
                    InterleaveTable::new(groups, |i: usize| pre[i], |node: u64| k.with(node).below(u64::MAX))
                })
                .collect();
        }
        drop(pools_o);
        lap("open market");
        // Husbands' segments: each husband cohort's men by wife cohort, in
        // its group's space, then in the open market.
        for h in 0..groups {
            for k in 0..years.len() {
                let s = b_min + k as i32;
                let (mut seg_in, mut seg_o) = ([0u64; GAPS + 1], [0u64; GAPS + 1]);
                for cs in 0..GAPS {
                    let yw = s - GAP_HI + cs as i32;
                    let (tin, to) = if ok(yw) {
                        let ri = spaces[h].row(at(yw));
                        let tin = ri[cs + 1] - ri[cs];
                        let to = if groups > 1 {
                            let ro = spaces[groups].row(at(yw));
                            let cat = cs * groups + h;
                            ro[cat + 1] - ro[cat]
                        } else {
                            0
                        };
                        (tin, to)
                    } else {
                        (0, 0)
                    };
                    seg_in[cs + 1] = seg_in[cs] + tin;
                    seg_o[cs + 1] = seg_o[cs] + to;
                }
                let c = &mut cells[h].coh[k];
                c.seg_in = seg_in;
                c.seg_o = seg_o;
                debug_assert!(seg_in[GAPS] + seg_o[GAPS] <= c.men());
            }
        }
        // The tables live on in the segments and interleaves.
        for s in &mut spaces {
            s.rows = Vec::new();
        }
        // q is clamped to [10⁻¹², 1 − 10⁻¹²]: 40 octaves reach it.
        let gq = |q: f64| procedural_core::dmath::pow(q / (1.0 - q), 1.0 / fu.shape);
        let union_q = [OctaveTable::new(1, 41, gq), OctaveTable::new(1, 41, |w| gq(1.0 - w))];
        // Dissolution shares per union year (`y + 1 + a0 + j` for wife class
        // `j` of cohort `y`).
        let sep_shares = (b_min + 1 + a0..=y1 + 1 + a0 + 140)
            .map(|uy| {
                let pmf = p.dissolution.class_pmf(uy, false);
                let mut w = [0.0f64; crate::params::MAX_CLASS + 1];
                w[..crate::params::MAX_CLASS].copy_from_slice(&pmf[1..]);
                w[crate::params::MAX_CLASS] = pmf[0];
                SystematicShares::new(&w)
            })
            .collect();
        let ncells = cells.len();
        let id_cell_bits = bits_for(ncells);
        let (ny, nb) = (years.len(), (y1 - p.y0 + 1).max(0) as usize);
        let (mut coh, mut cls, mut seps, mut bridges) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut infos = Vec::with_capacity(ncells);
        for cell in cells {
            assert_eq!(cell.coh.len(), ny);
            let base = cls.len() as u32;
            coh.extend(cell.coh.into_iter().map(|mut c| {
                c.cls_at += base;
                c
            }));
            cls.extend(cell.cls);
            seps.extend(cell.seps);
            bridges.extend(cell.bridges);
            infos.push(CellInfo { key: cell.key, group: cell.group });
        }
        assert!(cls.len() < u32::MAX as usize);
        let mut m = Mono {
            p,
            b_min,
            y1,
            a0,
            groups,
            cells: infos,
            ny,
            nb,
            coh,
            cls,
            seps,
            bridges,
            blocks: Vec::with_capacity(ncells * nb),
            block_top: Vec::with_capacity(ncells * nb),
            elig_rows: Vec::with_capacity(ncells * nb * AGES),
            elig_alive: Prefixes::Wide(Vec::new()),
            spaces,
            open_mix,
            open_seated,
            life,
            adult_shape,
            young_shape,
            sep_shares,
            union_q,
            id_index_bits: 53 - ID_YEAR_BITS - id_cell_bits,
            names: naming::NameCache::new(&p.name_data),
            residence: std::sync::OnceLock::new(),
            institutions: std::sync::OnceLock::new(),
            occupations: std::sync::OnceLock::new(),
            sample_share: std::sync::OnceLock::new(),
        };
        assert!(y1 - b_min < 1 << ID_YEAR_BITS, "a world spans at most 512 years");

        lap("segments");
        // 4. Birth blocks and eligible mothers, per cell. Eligible mothers
        // first (survival first: who is alive doesn't depend on births), then
        // each year's births split over (kind, mother age) under them: at
        // most one child per mother a year.
        // Rows per cell, concatenated.
        let per_cell: Vec<(Vec<EligRow>, Vec<u64>)> = erows.into_iter().zip(ealive).collect();
        assert!(per_cell.iter().all(|c| c.0.len() == nb * AGES));
        let mut alive = Vec::new();
        for (rows, a) in per_cell {
            assert!(alive.len() + a.len() < u32::MAX as usize);
            let base = alive.len() as u32;
            m.elig_rows.extend(rows.into_iter().map(|mut r| {
                r.off += base;
                r
            }));
            alive.extend(a);
        }
        // Every (cell, year)'s blocks, in parallel.
        let cys: Vec<(u16, i32)> = (0..ncells as u16).flat_map(|c| (p.y0..=y1).map(move |y| (c, y))).collect();
        let blocks: Vec<BlockRow> = cys
            .into_par_iter()
            .map(|(cell, y)| {
                let r = m.block_row(cell, y);
                let mut row = [u64::MAX; 64];
                row[..r.len()].copy_from_slice(&r);
                BlockRow(row)
            })
            .collect();
        m.block_top = blocks.iter().map(|r| std::array::from_fn(|g| r.0[8 * g])).collect();
        m.blocks = blocks;
        m.elig_alive = Prefixes::new(alive);
        lap("births");
        release_free_memory();
        m
    }

    #[inline]
    fn cell(&self, cell: u16) -> &CellInfo {
        &self.cells[cell as usize]
    }

    /// Cell cohort `(cell, y)`'s index in the per-cohort arrays.
    #[inline]
    fn ci(&self, cell: u16, y: i32) -> usize {
        cell as usize * self.ny + (y - self.b_min) as usize
    }

    /// Cell `cell`'s year `y ≥ y0` in the per-year arrays.
    #[inline]
    fn bi(&self, cell: u16, y: i32) -> usize {
        cell as usize * self.nb + (y - self.p.y0) as usize
    }

    #[inline]
    fn c(&self, cell: u16, y: i32) -> &Coh {
        &self.coh[self.ci(cell, y)]
    }

    /// Class `idx` of cell cohort `(cell, y)`'s constants.
    #[inline]
    fn crec(&self, cell: u16, y: i32, idx: usize) -> &ClassRec {
        &self.cls[self.c(cell, y).cls_at as usize + idx]
    }

    /// Cell cohort `(cell, y)`'s death-age knots: adults' or the young's, by
    /// sex.
    #[inline]
    fn knots(&self, y: i32, young: bool, female: bool) -> (&OctaveShape, &[f32]) {
        let (la, ly) = (self.adult_shape.len(), self.young_shape.len());
        let base = (y - self.b_min) as usize * 2 * (la + ly);
        let s = (!female) as usize;
        if young {
            let at = base + 2 * la + s * ly;
            (&self.young_shape, &self.life[at..at + ly])
        } else {
            let at = base + s * la;
            (&self.adult_shape, &self.life[at..at + la])
        }
    }

    /// The block holding position `q` of year `y`'s birth order in `cell`
    /// (the last whose start is at most `q`): one line of the tops, one of
    /// the row, no branches.
    #[inline]
    fn block_of(&self, cell: u16, y: i32, q: u64) -> usize {
        let k = self.bi(cell, y);
        let top = &self.block_top[k];
        let g = top[1..].iter().filter(|&&s| s <= q).count();
        let row = &self.blocks[k].0;
        8 * g + row[8 * g + 1..8 * g + 8].iter().filter(|&&s| s <= q).count()
    }

    /// The wife class of the wife with rank `k` (`k < wives`) in cohort `c`
    /// of `cell`: between its guide entries, by the class records' starts.
    #[inline]
    fn wife_class_idx(&self, c: &Coh, k: u64) -> usize {
        let g = (k >> c.wshift) as usize;
        let (mut a, mut b) = (c.wguide[g] as usize, c.wguide[g + 1] as usize);
        let recs = &self.cls[c.cls_at as usize + 4..c.cls_at as usize + 4 + c.nwc as usize];
        let i = c.young[0] + k;
        while a < b {
            let m = (a + b + 1) / 2;
            if recs[m].start <= i { a = m } else { b = m - 1 }
        }
        a
    }

    fn exists(&self, y: i32) -> bool {
        y >= self.b_min && y <= self.y1
    }

    /// Heap bytes by component (what the world keeps after its build).
    pub fn memory_report(&self) -> Vec<(&'static str, usize)> {
        vec![
            ("cohorts", self.coh.capacity() * std::mem::size_of::<Coh>()),
            ("union-age classes (mb)", self.coh.iter().map(|c| c.mb.capacity() * 8).sum()),
            ("death tables", self.life.capacity() * 4),
            ("class records", self.cls.capacity() * std::mem::size_of::<ClassRec>() + self.seps.capacity() * std::mem::size_of::<SepRec>()),
            ("interleaves", self.spaces.iter().map(|s| s.ilvs.iter().map(|t| t.heap_bytes()).sum::<usize>()).sum::<usize>() + self.open_mix.iter().map(|t| t.heap_bytes()).sum::<usize>()),
            ("union tables", self.spaces.iter().map(|s| s.rows.capacity() * 8).sum()),
            ("birth blocks", self.blocks.capacity() * std::mem::size_of::<BlockRow>() + self.block_top.capacity() * 64),
            ("eligible rows", self.elig_rows.capacity() * std::mem::size_of::<EligRow>()),
            ("alive prefixes", self.elig_alive.bytes()),
            ("bridges", self.bridges.capacity() * std::mem::size_of::<CompactPerm4>()),
            ("separation shares", self.sep_shares.iter().map(|s| s.len() * 8).sum()),
            ("name tables (built so far)", self.names.heap_bytes()),
        ]
        .into_iter()
        .chain(self.residence.get().map(|r| r.heap_parts()).into_iter().flatten())
        .chain(self.institutions.get().map(|i| ("schools and colleges", i.heap_bytes())))
        .chain(self.occupations.get().map(|o| ("occupations", o.heap_bytes())))
        .collect()
    }

    pub fn first_year(&self) -> i32 {
        self.b_min
    }

    pub fn last_year(&self) -> i32 {
        self.y1
    }

    /// Cells (heritage groups; one in a heritage-blind world).
    pub fn cells(&self) -> u16 {
        self.cells.len() as u16
    }

    /// The heritage group of a cell, `None` in a heritage-blind world.
    pub fn heritage(&self, cell: u16) -> Option<Heritage> {
        (self.groups > 1).then_some(Heritage(self.cell(cell).group as u8))
    }

    /// A person's heritage group, `None` in a heritage-blind world.
    pub fn heritage_of(&self, x: Pid) -> Option<Heritage> {
        self.heritage(x.cell)
    }

    /// The name of a person's heritage group (the pack's), `None` in a
    /// heritage-blind world.
    pub fn heritage_name(&self, x: Pid) -> Option<&'a str> {
        let p: &'a Params = self.p;
        self.heritage_of(x).map(|h| p.heritage.groups[h.0 as usize].name.as_str())
    }

    /// The person's external id: `cell · 2^(9 + b) + (y − first year) · 2^b
    /// + i`, with `b` index bits (53 − 9 − the cell bits). It stays below
    /// 2⁵³, so a JSON number carries it exactly.
    pub fn id(&self, x: Pid) -> u64 {
        debug_assert!(x.i < 1 << self.id_index_bits);
        ((x.cell as u64) << (ID_YEAR_BITS + self.id_index_bits)) | (((x.y - self.b_min) as u64) << self.id_index_bits) | x.i
    }

    /// The person with external id `id` (not checked against the world: see
    /// [`Self::contains`]).
    pub fn pid(&self, id: u64) -> Pid {
        let b = self.id_index_bits;
        Pid { cell: (id >> (ID_YEAR_BITS + b)).min(u16::MAX as u64) as u16, y: ((id >> b) & ((1 << ID_YEAR_BITS) - 1)) as i32 + self.b_min, i: id & ((1 << b) - 1) }
    }

    /// Whether `x` is a person of this world.
    pub fn contains(&self, x: Pid) -> bool {
        (x.cell as usize) < self.cells.len() && self.exists(x.y) && x.i < self.c(x.cell, x.y).n
    }

    /// Cell cohort `(cell, y)`'s size.
    pub fn cohort_n(&self, cell: u16, y: i32) -> u64 {
        if self.exists(y) && (cell as usize) < self.cells.len() { self.c(cell, y).n } else { 0 }
    }

    pub fn sex(&self, x: Pid) -> Sex {
        if x.i < self.c(x.cell, x.y).women { Sex::Female } else { Sex::Male }
    }

    /// The death class of `x`, its rank in it, and the class.
    pub fn class(&self, x: Pid) -> (Class, u64) {
        let c = self.c(x.cell, x.y);
        let female = x.i < c.women;
        let base = if female { 0 } else { c.women };
        let s = (!female) as usize;
        let r = x.i - base;
        let young = c.young[s];
        if r < young {
            return (Class { kind: Kind::Young, start: base, n: young, req: None }, r);
        }
        let r = r - young;
        if female {
            if r < c.wives {
                let j = self.wife_class_idx(c, r);
                let rec = &self.cls[c.cls_at as usize + 4 + j];
                return (Class { kind: Kind::Wife(j), start: rec.start, n: rec.n, req: Some(c.adult_req) }, x.i - rec.start);
            }
            return (self.never_class(x.cell, x.y, true), r - c.wives);
        }
        // Men (option A, `thinking/claude/010`): one adult class, the
        // cohort's life table from 16; a union whose partner is dead at its
        // start is void.
        (self.adult_men(x.cell, x.y), r)
    }

    /// A cell cohort's adult men: one death class.
    fn adult_men(&self, cell: u16, y: i32) -> Class {
        let c = self.c(cell, y);
        Class { kind: Kind::Never, start: c.women + c.young[1], n: c.men(), req: Some(c.adult_req) }
    }

    /// A class's slot permutation (its stored multiplier, keyed offset).
    #[inline]
    fn perm(&self, cell: u16, y: i32, female: bool, cl: &Class) -> AffinePerm {
        let r = self.crec(cell, y, class_index(cl.kind, female));
        debug_assert_eq!(r.n, cl.n);
        rec_perm(r)
    }

    /// Slot `rho` of `n` in an adult class: the death time.
    #[inline]
    fn slot_time(&self, y: i32, c: &Coh, female: bool, aft: [f32; 2], n: u64, rho: u64) -> i64 {
        let (shape, k) = self.knots(y, false, female);
        (c.mid + aft_secs(aft, shape.eval(k, slot_w(rho, n)))).max(c.adult_req + 1)
    }

    /// Slot `rho` of `n` in a young class: the age at death (seconds, ≥ 1).
    #[inline]
    fn young_secs(&self, y: i32, female: bool, n: u64, rho: u64) -> i64 {
        let (shape, k) = self.knots(y, true, female);
        ((shape.eval(k, slot_w(rho, n)) * YEAR) as i64).max(1)
    }

    /// The least slot `w` of a cell cohort's adults of one sex whose death
    /// time is at most `t` (every adult class shares the table and
    /// requirement): a class's slots dead by `t` are those with `w` at or
    /// above it.
    fn adult_dead_w(&self, cell: u16, y: i32, female: bool, t: i64) -> Option<f64> {
        let c = self.c(cell, y);
        if t <= c.adult_req {
            return None;
        }
        // The age turns about where a·YEAR reaches t − mid + 1 (the table's
        // value before the group's scaling).
        let aft = self.crec(cell, y, class_index(Kind::Never, female)).aft;
        let v = ((t - c.mid + 1) as f64 - aft[0] as f64) / aft[1] as f64;
        let (shape, k) = self.knots(y, false, female);
        shape.threshold_near(k, v, |a| (c.mid + aft_secs(aft, a)).max(c.adult_req + 1) <= t)
    }

    /// The least slot `w` of a cell cohort's young of one sex whose age at
    /// death (seconds) is at most `lim`.
    fn young_dead_w(&self, y: i32, female: bool, lim: i64) -> Option<f64> {
        let (shape, k) = self.knots(y, true, female);
        shape.threshold_near(k, (lim + 1) as f64 / YEAR, |a| ((a * YEAR) as i64).max(1) <= lim)
    }

    /// Members of an absolute class dead by `t`: the slots whose time is
    /// at most `t` (one threshold on the shared table, then exact counts).
    pub fn dead_by(&self, cell: u16, y: i32, female: bool, cl: &Class, t: i64) -> u64 {
        if cl.n == 0 {
            return 0;
        }
        self.adult_dead_w(cell, y, female, t).map_or(0, |w| count_from_threshold(cl.n, w))
    }

    /// The birth time: the mother's phase in the year, turned by the year
    /// ([`year_turn`]), so siblings are at least 330 days apart; founders at
    /// their own keyed phase.
    pub fn birth(&self, x: Pid) -> i64 {
        if x.y < self.p.y0 {
            return self.at_phase(x.cell, x.y, self.cell(x.cell).key.with3(T_PHASE, x.y as u64 | 1 << 40, x.i).below(Q));
        }
        self.birth_with(x, &self.bridge(x.cell, x.y))
    }

    /// The time at `phase` (units of 2⁻³²) of year `y`.
    #[inline]
    fn at_phase(&self, cell: u16, y: i32, phase: u64) -> i64 {
        // The cohort's stored year start and length (the same values).
        let c = self.c(cell, y);
        // phase < 2³² and a year < 2²⁶ s: the product fits 64 bits.
        (c.mid - HALF_YEAR) + ((phase * c.ylen as u64) >> 32) as i64
    }

    /// The largest phase of year `y` whose time is at most `t`.
    fn phase_upto(&self, y: i32, t: i64) -> Option<u64> {
        let (a, b) = (year_start(y), year_start(y + 1));
        if t < a {
            return None;
        }
        if t >= b - 1 {
            return Some(Q - 1);
        }
        // (p·len) >> 32 ≤ t − a ⟺ p·len < (t − a + 1)·2³².
        let len = (b - a) as u128;
        Some(((((t - a + 1) as u128) << 32).div_ceil(len) - 1).min(Q as u128 - 1) as u64)
    }

    /// A mother's phase before the year's turn ([`year_turn`]): her slot in
    /// her class, rotated by the class's key, so the alive slots of a class
    /// with phase up to `p` are at most two runs ([`phase_runs`]).
    fn phase(&self, cell: u16, ym: i32, cl: &Class, rho: u64) -> u64 {
        let h = mul_div(2 * rho + 1, Q, 2 * cl.n);
        (h + self.phase_beta(cell, ym, cl)) % Q
    }

    #[inline]
    fn phase_beta(&self, cell: u16, ym: i32, cl: &Class) -> u64 {
        let idx = match cl.kind {
            Kind::Young => 0,
            Kind::Never => 2,
            Kind::Wife(j) => 4 + j,
        };
        self.crec(cell, ym, idx).beta
    }

    pub fn death(&self, x: Pid) -> i64 {
        let c = self.c(x.cell, x.y);
        let female = x.i < c.women;
        let (cl, r) = self.class(x);
        let rec = &self.cls[c.cls_at as usize + class_index(cl.kind, female)];
        let rho = rec_fwd(rec, r);
        match cl.req {
            Some(_) => self.slot_time(x.y, c, female, rec.aft, cl.n, rho),
            None => self.birth(x) + self.young_secs(x.y, female, cl.n, rho),
        }
    }

    /// Prototype aid (research/2026-10-04-workplace-rosters.md): year
    /// `y`'s eligible mothers of `cell` by (kind, age), recomputed from the
    /// class sizes and one death threshold per mother cohort (what computed
    /// classes would do per lookup), instead of read from the stored rows.
    #[doc(hidden)]
    pub fn eligible_from_scratch(&self, cell: u16, y: i32) -> [u64; 2 * AGES] {
        let mut out = [0u64; 2 * AGES];
        let end = year_start(y + 1);
        for a in A_LO..=A_HI {
            let ym = y - a;
            if !self.exists(ym) {
                continue;
            }
            let c = self.c(cell, ym);
            let th = self.adult_dead_w(cell, ym, true, end);
            let alive = |n: u64| n - th.map_or(0, |w| count_from_threshold(n, w));
            let mc = self.married_classes(cell, ym, a);
            let (mut married, mut single) = (0, 0);
            for j in 0..c.nwc as usize {
                let k = alive(c.db(j + 1) - c.db(j));
                if j < mc { married += k } else { single += k }
            }
            single += alive(c.women - c.young[0] - c.wives);
            out[(a - A_LO) as usize] = married;
            out[AGES + (a - A_LO) as usize] = single;
        }
        out
    }

    /// Prototype aid: whether [`Self::eligible_from_scratch`] agrees with the
    /// stored rows.
    #[doc(hidden)]
    pub fn eligible_stored(&self, cell: u16, y: i32) -> [u64; 2 * AGES] {
        let mut out = [0u64; 2 * AGES];
        for a in A_LO..=A_HI {
            for k in 0..2 {
                out[k * AGES + (a - A_LO) as usize] = self.eligible(cell, y, k, a);
            }
        }
        out
    }

    /// Profiling aid: mean ns of each internal step over `xs` (throughput,
    /// one pass per step).
    #[doc(hidden)]
    pub fn profile_parts(&self, xs: &[Pid]) -> Vec<(&'static str, f64)> {
        use std::hint::black_box;
        use std::time::Instant;
        let mut out = Vec::new();
        let mut time = |name: &'static str, f: &mut dyn FnMut(Pid) -> u64| {
            let s = Instant::now();
            let mut h = 0u64;
            for &x in xs {
                h = h.wrapping_add(f(x));
            }
            black_box(h);
            out.push((name, s.elapsed().as_nanos() as f64 / xs.len() as f64));
        };
        time("sex", &mut |x| (self.sex(x) == Sex::Female) as u64);
        time("class", &mut |x| self.class(x).1);
        time("perm (class + build)", &mut |x| {
            let (cl, _) = self.class(x);
            self.perm(x.cell, x.y, self.sex(x) == Sex::Female, &cl).parts().1
        });
        time("perm.fwd (class + build + fwd)", &mut |x| {
            let (cl, r) = self.class(x);
            self.perm(x.cell, x.y, self.sex(x) == Sex::Female, &cl).fwd(r)
        });
        time("slot_time only (adults)", &mut |x| {
            let female = self.sex(x) == Sex::Female;
            let (cl, r) = self.class(x);
            match cl.req {
                Some(_) => self.slot_time(x.y, self.c(x.cell, x.y), female, self.crec(x.cell, x.y, class_index(cl.kind, female)).aft, cl.n, r) as u64,
                None => 0,
            }
        });
        time("wife_union (wives)", &mut |x| {
            let c = self.c(x.cell, x.y);
            if x.i < c.women && x.i >= c.young[0] && x.i - c.young[0] < c.wives {
                self.wife_union(x).1 as u64
            } else {
                0
            }
        });
        time("death", &mut |x| self.death(x) as u64);
        time("bridge (build + inv)", &mut |x| self.bridge(x.cell, x.y).inv(x.i));
        time("mother_class_slot", &mut |x| self.mother_class_slot(x, &self.bridge(x.cell, x.y)).map_or(0, |m| m.3));
        time("birth", &mut |x| self.birth(x) as u64);
        time("mother", &mut |x| self.mother(x).map_or(0, |m| m.i));
        time("partner_seat", &mut |x| self.partner_seat(x).map_or(0, |p| p.1 as u64));
        time("spouse", &mut |x| self.spouse(x).map_or(0, |p| p.1 as u64));
        time("father", &mut |x| self.father(x).map_or(0, |p| p.i));
        out
    }

    pub fn alive_at(&self, x: Pid, t: i64) -> bool {
        self.birth(x) <= t && t < self.death(x)
    }

    /// A wife's union age class `j` and her union time.
    fn wife_union(&self, w: Pid) -> (usize, i64) {
        let c = self.c(w.cell, w.y);
        let k = w.i - c.young[0];
        let j = c.mb.partition_point(|&b| b <= k) - 1;
        let fu = &self.p.unions.first_union;
        let q = ((k as f64 + 0.5) / c.wives as f64 * c.top).clamp(1e-12, 1.0 - 1e-12);
        let lo = (self.a0 + j as i32) as f64;
        let g = if q < 0.5 { self.union_q[0].eval(q) } else { self.union_q[1].eval(1.0 - q) };
        let age = (fu.origin + (c.median - fu.origin) * g).clamp(lo, lo + 1.0 - 1e-6);
        (j, year_start(w.y + 1) + (age * YEAR) as i64)
    }

    /// Wife class `j` of a cell cohort's separation map `r ↦ (a²·r + b) mod
    /// n`: the death map's multiplier squared, so that both the map itself
    /// and its composition with the inverse death map (multiplier `a`) are
    /// well spread, and any joint count of death slot and separation
    /// category in the class is one affine image (`thinking/claude/009`).
    /// Returns `(a², b, class)`; forward only, so no inverse is needed.
    #[inline]
    fn sep_map(&self, cell: u16, y: i32, j: usize) -> (u64, u64, Class) {
        let cl = self.wife_class(cell, y, j);
        let n = cl.n.max(1);
        let at = self.c(cell, y).cls_at as usize + 4 + j;
        let a = self.cls[at].a;
        let a2 = (mul_mod(a, a, n)).max(if n == 1 { 0 } else { 1 });
        (a2, self.seps[at].b, cl)
    }

    /// The separation category of position `v` of wife class `j`'s
    /// separation image (`MAX_CLASS`: none): `[S₁, …, S₄₀, none]`, sizes by a
    /// keyed systematic rounding of the pack's dissolution classes for the
    /// union year (stored per year).
    #[inline]
    fn sep_cat(&self, cell: u16, y: i32, j: usize, n: u64, v: u64) -> usize {
        let u = self.seps[self.c(cell, y).cls_at as usize + 4 + j].u;
        self.sep_shares[(y + j as i32 - self.b_min) as usize].part_of_offset(n, u, v)
    }

    /// Where the first `d` separation categories of wife class `j` end in
    /// its separation image.
    fn sep_end(&self, cell: u16, y: i32, j: usize, n: u64, d: usize) -> u64 {
        if d == 0 {
            return 0;
        }
        let u = self.seps[self.c(cell, y).cls_at as usize + 4 + j].u;
        self.sep_shares[(y + j as i32 - self.b_min) as usize].end_of_offset(n, u, d - 1)
    }

    /// A wife's separation category, or `MAX_CLASS` (none).
    fn wife_sep_cat(&self, w: Pid, k: u64, c: &Coh) -> usize {
        let j = c.death_class_of(c.mb.partition_point(|&b| b <= k) - 1);
        let (a2, b, cl) = self.sep_map(w.cell, w.y, j);
        let r = k - (cl.start - c.young[0]);
        let v = (mul_mod(a2, r, cl.n) + b) % cl.n;
        self.sep_cat(w.cell, w.y, j, cl.n, v)
    }

    /// A first union's separation time, if it separates: in its `d`-th
    /// year, at a keyed phase. From the wife.
    pub fn separation(&self, w: Pid) -> Option<i64> {
        let (_, start) = self.spouse(w)?;
        self.sep_time(w, start)
    }

    /// A wife's separation time from her union's start, void or not.
    fn sep_time(&self, w: Pid, start: i64) -> Option<i64> {
        let c = self.c(w.cell, w.y);
        let k = w.i.checked_sub(c.young[0])?;
        if w.i >= c.women || k >= c.wives {
            return None;
        }
        let d = self.wife_sep_cat(w, k, c);
        if d >= crate::params::MAX_CLASS {
            return None;
        }
        let f = self.cell(w.cell).key.with3(T_SEP, w.y as u64, w.i).with(2).unit();
        Some(start + ((d as f64 + f) * YEAR) as i64)
    }

    /// Wife classes of a cell cohort, and class `j`'s members (life
    /// indices).
    pub fn wife_classes(&self, cell: u16, y: i32) -> usize {
        self.c(cell, y).nwc as usize
    }

    pub fn wife_class_members(&self, cell: u16, y: i32, j: usize) -> std::ops::Range<u64> {
        let cl = self.wife_class(cell, y, j);
        cl.start..cl.start + cl.n
    }

    /// A wife's separation category index `d` (0-based: the `(d+1)`-th
    /// year), if she separates.
    pub fn separation_class(&self, w: Pid) -> Option<usize> {
        let c = self.c(w.cell, w.y);
        let k = w.i.checked_sub(c.young[0])?;
        if w.i >= c.women || k >= c.wives {
            return None;
        }
        let d = self.wife_sep_cat(w, k, c);
        (d < crate::params::MAX_CLASS).then_some(d)
    }

    /// The first `d` with `first + d` whole years at most `t`: the
    /// separation categories surely past by `t` in wife class `j`.
    pub fn sep_whole_years(&self, y: i32, j: usize, t: i64) -> usize {
        let first = year_start(y + 1) + ((self.a0 + j as i32 + 1) as f64 * YEAR) as i64;
        let mut d = 0usize;
        while d < crate::params::MAX_CLASS && first + ((d + 1) as f64 * YEAR) as i64 <= t {
            d += 1;
        }
        d
    }

    /// How a first union ends: at the separation, or the first death,
    /// whichever comes first (competing risks). `(time, how)`, how: 0
    /// separation, 1 the husband's death, 2 the wife's.
    pub fn union_end(&self, x: Pid) -> Option<(i64, u8)> {
        self.union_of(x).map(|u| (u.end, u.how))
    }

    /// `x`'s first union, if not void: the partners, its start and its end
    /// (as [`Self::union_end`]), each lookup done once.
    pub fn union_of(&self, x: Pid) -> Option<Union> {
        let (p, start) = self.partner_seat(x)?;
        let (wife, husband) = if self.sex(x) == Sex::Female { (x, p) } else { (p, x) };
        self.union_from(wife, husband, start, self.death(wife), self.death(husband))
    }

    /// The union of a seated couple given both deaths (`None`: void).
    fn union_from(&self, wife: Pid, husband: Pid, start: i64, dw: i64, dh: i64) -> Option<Union> {
        if dw <= start || dh <= start {
            return None;
        }
        let mut end = (dh, 1u8);
        if dw < end.0 {
            end = (dw, 2);
        }
        if let Some(s) = self.sep_time(wife, start) {
            if s < end.0 {
                end = (s, 0);
            }
        }
        Some(Union { wife, husband, start, end: end.0, how: end.1 })
    }

    /// A child's mother, birth time, and (if the mother's partner is the
    /// father) the father and their union: [`Self::mother`],
    /// [`Self::birth`] and [`Self::father`] in one walk.
    pub fn parents(&self, x: Pid) -> Option<(Pid, i64, Option<Union>)> {
        let bridge = self.bridge(x.cell, x.y);
        let (ym, married, ci, rho) = self.mother_class_slot(x, &bridge)?;
        let r = self.crec(x.cell, ym, ci);
        let m = Pid { cell: x.cell, y: ym, i: r.start + rec_inv(r, rho) };
        let birth = self.at_phase(x.cell, x.y, rec_phase(r, rho, x.y));
        if !married {
            return Some((m, birth, None));
        }
        let conception = birth - GESTATION;
        let Some((h, start)) = self.partner_seat(m) else { return Some((m, birth, None)) };
        let (dh, dm) = (self.death(h), self.death(m));
        if dh <= conception.max(start) || dm <= start {
            return Some((m, birth, None));
        }
        let u = self.union_from(m, h, start, dm, dh).expect("both alive at the start");
        if u.how == 0 && u.end <= conception {
            return Some((m, birth, None));
        }
        Some((m, birth, Some(u)))
    }

    /// Wives of class `j` of a cell cohort whose union has separated by `t`
    /// and who are alive at `t`: one affine-image count (floor sums).
    pub fn separated_alive(&self, cell: u16, y: i32, j: usize, t: i64) -> u64 {
        let (a2, bs, cl) = self.sep_map(cell, y, j);
        if cl.n == 0 {
            return 0;
        }
        let pw = self.perm(cell, y, true, &cl);
        let dead = self.dead_by(cell, y, true, &cl, t);
        // Separated by `t` (by category: the d-th year ends before `t` is
        // sure; the year containing `t` depends on each union's date, so
        // count only whole years: categories with start + d years ≤ t).
        let first = year_start(y + 1) + ((self.a0 + j as i32 + 1) as f64 * YEAR) as i64;
        let mut sep_hi = 0usize;
        while sep_hi < crate::params::MAX_CLASS && first + ((sep_hi + 1) as f64 * YEAR) as i64 <= t {
            sep_hi += 1;
        }
        // Death slots ρ ∈ [dead, n) ⟺ v_w ∈ [dead, n); separation image of
        // the member with death slot v: π_s(π_w⁻¹(v)) = (a₂ a⁻¹ (v − b_w) + b_s)
        // mod n, affine in v.
        let (_, bw) = pw.parts();
        let n = cl.n as u128;
        let ainv = pw.inverse_multiplier() as u128;
        let m = ((a2 as u128 * ainv) % n).max(if n == 1 { 0 } else { 1 });
        let c = (bs as u128 + n * n - (m * bw as u128) % n) % n;
        let sigma = AffinePerm::with_parts(cl.n, m as u64, c as u64);
        sigma.count(dead, cl.n, 0, self.sep_end(cell, y, j, cl.n, sep_hi))
    }

    /// The partner and the union's start, if `x` partners: the union is
    /// void (for both) if either partner is dead at its start (option A).
    pub fn spouse(&self, x: Pid) -> Option<(Pid, i64)> {
        let (p, start) = self.partner_seat(x)?;
        (self.death(x) > start && self.death(p) > start).then_some((p, start))
    }

    /// The partner of `x`'s first-union seat and its start, void or not.
    pub fn partner_seat(&self, x: Pid) -> Option<(Pid, i64)> {
        let c = self.c(x.cell, x.y);
        let k0 = (x.y - self.b_min) as usize;
        if x.i < c.women {
            let k = x.i.checked_sub(c.young[0])?;
            if k >= c.wives {
                return None;
            }
            let start = self.wife_union(x).1;
            let (kg, open) = if c.open_t > 0 { c.open().count_member(k) } else { (0, false) };
            if open {
                // Open market: her rank among the year's open wives, then
                // the open space's layout gives the category (c, h).
                let r = self.open_mix[k0].select(x.cell as usize, kg);
                if r >= self.open_seated[k0] {
                    return None;
                }
                let (cat, j) = self.spaces[self.groups].ilvs[k0].locate(r);
                let (cs, hc) = (cat / self.groups, (cat % self.groups) as u16);
                let s = x.y + GAP_HI - cs as i32;
                let h = self.c(hc, s);
                return Some((Pid { cell: hc, y: s, i: h.women + h.young[1] + h.seg_in[GAPS] + h.seg_o[cs] + j }, start));
            }
            let r = k - kg;
            let (cs, j) = self.spaces[x.cell as usize].ilvs[k0].locate(r);
            let s = x.y + GAP_HI - cs as i32;
            let h = self.c(x.cell, s);
            return Some((Pid { cell: x.cell, y: s, i: h.women + h.young[1] + h.seg_in[cs] + j }, start));
        }
        let r = (x.i - c.women).checked_sub(c.young[1])?;
        if r < c.seg_in[GAPS] {
            let cs = c.seg_in.partition_point(|&b| b <= r) - 1;
            let yw = x.y - GAP_HI + cs as i32;
            let rr = self.spaces[x.cell as usize].ilvs[(yw - self.b_min) as usize].select(cs, r - c.seg_in[cs]);
            let cw = self.c(x.cell, yw);
            let k = if cw.open_t > 0 { cw.open().select_out(rr) } else { rr };
            let w = Pid { cell: x.cell, y: yw, i: cw.young[0] + k };
            return Some((w, self.wife_union(w).1));
        }
        let r = r - c.seg_in[GAPS];
        if r >= c.seg_o[GAPS] {
            return None;
        }
        let cs = c.seg_o.partition_point(|&b| b <= r) - 1;
        let yw = x.y - GAP_HI + cs as i32;
        let kw = (yw - self.b_min) as usize;
        let rr = self.spaces[self.groups].ilvs[kw].select(cs * self.groups + x.cell as usize, r - c.seg_o[cs]);
        let (g, kg) = self.open_mix[kw].locate(rr);
        let cw = self.c(g as u16, yw);
        let w = Pid { cell: g as u16, y: yw, i: cw.young[0] + cw.open().select(kg) };
        Some((w, self.wife_union(w).1))
    }

    // ---- births ----

    /// The share of a year's births to single mothers: the pack's non-union
    /// births per woman against its union births.
    fn single_share(&self, y: i32) -> f64 {
        let f = &self.p.fertility;
        let nu = f.nonunion_count_pmf(y - 25, 1.0);
        let e = nu[1] + 2.0 * nu[2];
        e / (e + f.union_births)
    }

    /// Year `y`'s births in `cell` over blocks (kind, mother age): a keyed
    /// systematic split by the era's shares, capped by each block's
    /// eligible mothers, so no mother has two children in a year. Block
    /// starts, in (kind, age) order, then the end.
    fn block_row(&self, cell: u16, y: i32) -> [u64; 2 * AGES + 1] {
        let n = self.c(cell, y).n;
        let s1 = self.single_share(y);
        let cum = |kind: usize, a: f64| if kind == 0 { (1.0 - s1) * mother_age_cdf(self.p, y, 0, a) } else { (1.0 - s1) + s1 * mother_age_cdf(self.p, y, 1, a) };
        let mut w = [0.0f64; 2 * AGES];
        let mut cap = [0u64; 2 * AGES];
        for i in 0..2 * AGES {
            let (kind, a) = (i / AGES, A_LO + (i % AGES) as i32);
            w[i] = cum(kind, a as f64 + 1.0) - cum(kind, a as f64);
            cap[i] = self.eligible(cell, y, kind, a);
        }
        let room: u64 = cap.iter().sum();
        assert!(room >= n, "cell {cell}, {y}: {n} births, {room} eligible mothers");
        let mut parts = [0u64; 2 * AGES];
        if n > 0 {
            capped_split(n, &w, &cap, self.cell(cell).key.with2(T_BLOCK, y as u64), &mut parts);
        }
        let mut row = [0u64; 2 * AGES + 1];
        for i in 0..2 * AGES {
            row[i + 1] = row[i] + parts[i];
        }
        row
    }

    /// The wife class and never-married classes of a cell cohort's women.
    fn wife_class(&self, cell: u16, y: i32, j: usize) -> Class {
        let c = self.c(cell, y);
        let rec = &self.cls[c.cls_at as usize + 4 + j];
        Class { kind: Kind::Wife(j), start: rec.start, n: rec.n, req: Some(c.adult_req) }
    }

    fn never_class(&self, cell: u16, y: i32, female: bool) -> Class {
        let c = self.c(cell, y);
        let (start, n) = if female {
            (c.young[0] + c.wives, c.women - c.young[0] - c.wives)
        } else {
            let husbands = c.seg_in[GAPS] + c.seg_o[GAPS];
            (c.women + c.young[1] + husbands, c.men() - husbands)
        };
        Class { kind: Kind::Never, start, n, req: Some(c.adult_req) }
    }

    /// Classes of wives married before age `a` (union age class below it).
    fn married_classes(&self, cell: u16, ym: i32, a: i32) -> usize {
        ((a - self.a0).max(0) as usize).min(self.c(cell, ym).nwc as usize)
    }

    /// The alive prefix over wife classes and the never-partnered alive of
    /// (year, mother age).
    #[inline]
    fn elig(&self, cell: u16, y: i32, a: i32) -> (Vec<u64>, u64) {
        let r = self.row(cell, y, a);
        let ym = y - a;
        let len = if self.exists(ym) { self.c(cell, ym).nwc as usize + 1 } else { 1 };
        let pre = &self.elig_alive;
        ((0..len).map(|j| pre.get(r.off as usize + j)).collect(), r.never_alive())
    }

    #[inline]
    fn row(&self, cell: u16, y: i32, a: i32) -> &EligRow {
        &self.elig_rows[self.bi(cell, y) * AGES + (a - A_LO) as usize]
    }

    /// Eligible mothers of (year, kind, age).
    #[inline]
    fn eligible(&self, cell: u16, y: i32, kind: usize, a: i32) -> u64 {
        self.row(cell, y, a).elig[kind]
    }

    /// The kind whose mothers a block's children go to: its own, or the
    /// other when its own has none (`None`: neither has any).
    #[inline]
    fn effective(&self, cell: u16, y: i32, kind: usize, a: i32) -> Option<usize> {
        effective_kind(&self.row(cell, y, a).elig, kind)
    }

    #[inline]
    fn bridge(&self, cell: u16, y: i32) -> CompactPerm4 {
        self.bridges[self.ci(cell, y)]
    }

    /// The mother: the eligible woman a child's block position maps to.
    pub fn mother(&self, x: Pid) -> Option<Pid> {
        self.mother_slot(x).map(|m| m.0)
    }

    /// The eligible mothers of year `y`, age `a`, kind `k`, in list order:
    /// runs of one class's slots, each `(class, first slot, first index)`.
    /// Married: wife classes below `a`, their alive slots. Single: wife
    /// classes from `a` on (all alive), then the never-married's alive slots.
    fn runs(&self, cell: u16, y: i32, k: usize, a: i32) -> Vec<(Class, u64, u64)> {
        let ym = y - a;
        let (e_alive, e_never) = self.elig(cell, y, a);
        let mc = self.married_classes(cell, ym, a);
        let mut v = Vec::new();
        let (lo, hi, base) = if k == 0 { (0, mc, 0) } else { (mc, e_alive.len() - 1, e_alive[mc]) };
        for j in lo..hi {
            let cl = self.wife_class(cell, ym, j);
            v.push((cl, cl.n - (e_alive[j + 1] - e_alive[j]), e_alive[j] - base));
        }
        if k == 1 {
            let nc = self.never_class(cell, ym, true);
            v.push((nc, nc.n - e_never, e_alive[e_alive.len() - 1] - base));
        }
        v
    }

    /// The mother, whether she was married before the child's year, her
    /// class and her slot in it.
    fn mother_slot(&self, x: Pid) -> Option<(Pid, bool, usize, u64)> {
        let (ym, married, ci, rho) = self.mother_class_slot(x, &self.bridge(x.cell, x.y))?;
        let r = self.crec(x.cell, ym, ci);
        Some((Pid { cell: x.cell, y: ym, i: r.start + rec_inv(r, rho) }, married, ci, rho))
    }

    /// The mother's cohort, marriage, class and slot (her identity takes
    /// one more permutation step): what a birth time needs. Mothers are in
    /// the child's cell.
    #[inline(always)]
    fn mother_class_slot(&self, x: Pid, bridge: &CompactPerm4) -> Option<(i32, bool, usize, u64)> {
        if x.y < self.p.y0 {
            return None;
        }
        let row = &self.blocks[self.bi(x.cell, x.y)].0;
        // The row's lines, and the child's cohort line (its year's start and
        // length), load while the bridge computes.
        for l in 0..8 {
            prefetch(&row[8 * l]);
        }
        prefetch(self.c(x.cell, x.y));
        let q = bridge.inv(x.i);
        let b = self.block_of(x.cell, x.y, q);
        let (kind, a) = (b / AGES, A_LO + (b % AGES) as i32);
        let (j, c) = (q - row[b], row[b + 1] - row[b]);
        // c's reciprocal is ready by the time the eligible row arrives.
        let c_inv = u64::MAX / c;
        let er = self.row(x.cell, x.y, a);
        let k = effective_kind(&er.elig, kind)?;
        let idx = match j.checked_mul(er.elig[k]) {
            Some(p) => procedural_core::perm::divmod_by_inverse(p, c, c_inv).0,
            None => mul_div(j, er.elig[k], c),
        };
        let ym = x.y - a;
        // The row's alive prefix (only entries the guide brackets are read).
        let (pre, off) = (&self.elig_alive, er.off as usize);
        // Single mothers count from the married ones' end (`elig[0]`).
        let at = if k == 0 { idx } else { er.elig[0] + idx };
        if k == 0 || at < er.total {
            // Wife class `cj`: its alive slots after its dead (married:
            // classes below `mc`; single: from `mc` on). The last class
            // whose prefix is at most `at`, between the guide's bounds; its
            // record is usually the lower bound's, so that load starts early.
            let g = (at >> er.gshift) as usize;
            let (lo, hi) = (er.guide[g] as usize, er.guide[g + 1] as usize);
            prefetch(self.crec(x.cell, ym, 4 + lo));
            if hi > lo {
                prefetch(self.crec(x.cell, ym, 5 + lo));
            }
            let cj = pre.last_le(off, lo, hi, at);
            let r = self.crec(x.cell, ym, 4 + cj);
            let (p0, p1) = (pre.get(off + cj), pre.get(off + cj + 1));
            Some((ym, k == 0, 4 + cj, r.n - (p1 - p0) + at - p0))
        } else {
            let r = self.crec(x.cell, ym, 2);
            Some((ym, false, 2, r.n - er.never_alive() + at - er.total))
        }
    }

    /// [`Self::birth`] with the cohort's bridge given.
    fn birth_with(&self, x: Pid, bridge: &CompactPerm4) -> i64 {
        let phase = match self.mother_class_slot(x, bridge) {
            Some((ym, _, ci, rho)) => rec_phase(self.crec(x.cell, ym, ci), rho, x.y),
            None => self.cell(x.cell).key.with3(T_PHASE, x.y as u64 | 1 << 40, x.i).below(Q),
        };
        self.at_phase(x.cell, x.y, phase)
    }

    /// A person's children, in birth-year order: a woman's from the birth
    /// blocks; a man's are his wife's whose father he is (married births
    /// conceived while their union holds and he lives).
    pub fn children(&self, x: Pid) -> Vec<Pid> {
        let mut out = Vec::new();
        if x.i < self.c(x.cell, x.y).women {
            self.each_child(x, |c, _| out.push(c));
            return out;
        }
        let Some((w, start)) = self.partner_seat(x) else { return out };
        let dh = self.death(x);
        if dh <= start || self.death(w) <= start {
            return out;
        }
        let sep = self.sep_time(w, start);
        let (cl, r) = self.class(w);
        let ph = self.phase(w.cell, w.y, &cl, self.perm(w.cell, w.y, true, &cl).fwd(r));
        self.each_child(w, |c, married| {
            if married {
                let conception = self.at_phase(c.cell, c.y, (ph + year_turn(c.y)) % Q) - GESTATION;
                if dh > conception.max(start) && sep.is_none_or(|s| s > conception) {
                    out.push(c);
                }
            }
        });
        out
    }

    /// The other children of `x`'s parents (here: of the mother; with one
    /// union each, a father's children are all his wife's).
    pub fn siblings(&self, x: Pid) -> Vec<Pid> {
        match self.mother(x) {
            Some(m) => self.children(m).into_iter().filter(|&c| c != x).collect(),
            None => Vec::new(),
        }
    }

    /// A woman's children in birth-year order, each with whether she was
    /// married (her block's kind) that year.
    fn each_child(&self, m: Pid, mut f: impl FnMut(Pid, bool)) {
        let c = self.c(m.cell, m.y);
        if m.i >= c.women || m.i < c.young[0] {
            return;
        }
        let (cl, r) = self.class(m);
        let rec = &self.cls[c.cls_at as usize + class_index(cl.kind, true)];
        let rho = rec_fwd(rec, r);
        // She is among a year's eligible while alive through it: her death
        // time is after the year's end (as the alive prefixes count), so
        // through the year before the one holding her last second.
        let last_year = procedural_core::stream::year_of(self.slot_time(m.y, c, true, rec.aft, cl.n, rho) - 1) - 1;
        // Her index among the eligible of her kind is the alive prefix past
        // her class less the slots after hers (`n − ρ`): one entry.
        let after = cl.n - rho;
        let nwc = c.nwc as usize;
        for a in A_LO..=A_HI {
            let y = m.y + a;
            if y < self.p.y0 {
                continue;
            }
            if y > self.y1.min(last_year) {
                break;
            }
            let er = self.row(m.cell, y, a);
            let mc = ((a - self.a0).max(0) as usize).min(nwc);
            let (kind, idx) = match cl.kind {
                Kind::Wife(j) => {
                    let next = self.elig_alive.get(er.off as usize + j + 1);
                    if j < mc { (0, next - after) } else { (1, next - er.elig[0] - after) }
                }
                _ => (1, er.total - er.elig[0] + er.never_alive() - after),
            };
            let m_n = er.elig[kind];
            let row = &self.blocks[self.bi(m.cell, y)].0;
            let bridge = self.bridge(m.cell, y);
            for bk in [kind, 1 - kind] {
                if effective_kind(&er.elig, bk) != Some(kind) {
                    continue;
                }
                let b = bk * AGES + (a - A_LO) as usize;
                let (s, cnt) = (row[b], row[b + 1] - row[b]);
                let lo = mul_div_ceil(idx, cnt, m_n);
                let hi = mul_div_ceil(idx + 1, cnt, m_n);
                for j in lo..hi {
                    f(Pid { cell: m.cell, y, i: bridge.fwd(s + j) }, kind == 0);
                }
            }
        }
    }

    /// A cell cohort's children born by `t`, as ranges of birth order: per
    /// block and run, the mothers whose phase is at most `t`'s, then their
    /// children (a contiguous range each).
    fn born_by(&self, cell: u16, y: i32, t: i64, mut f: impl FnMut(std::ops::Range<u64>)) {
        let Some(p) = self.phase_upto(y, t) else { return };
        for b in 0..2 * AGES {
            self.born_in_block(cell, y, p, b, &mut f);
        }
    }

    /// A cell cohort's children born by `t`, counted (blocks in parallel).
    fn born_by_par(&self, cell: u16, y: i32, t: i64) -> u64 {
        let Some(p) = self.phase_upto(y, t) else { return 0 };
        (0..2 * AGES)
            .into_par_iter()
            .map(|b| {
                let mut k = 0;
                self.born_in_block(cell, y, p, b, &mut |r: std::ops::Range<u64>| k += r.end - r.start);
                k
            })
            .sum()
    }

    /// [`Self::born_by`] for one block `b` (kind × age), given the phase
    /// `p` of the time in the year.
    fn born_in_block(&self, cell: u16, y: i32, p: u64, b: usize, f: &mut impl FnMut(std::ops::Range<u64>)) {
        let row = &self.blocks[self.bi(cell, y)].0;
        let (bk, a) = (b / AGES, A_LO + (b % AGES) as i32);
        let (s, cnt) = (row[b], row[b + 1] - row[b]);
        if cnt == 0 {
            return;
        }
        let Some(k) = self.effective(cell, y, bk, a) else { return };
        let m = self.eligible(cell, y, k, a) as u128;
        for (cl, lo, at) in self.runs(cell, y, k, a) {
            for run in phase_runs(cl.n, (self.phase_beta(cell, y - a, &cl) + year_turn(y)) % Q, p, lo) {
                if run.start >= run.end {
                    continue;
                }
                let (i0, i1) = (at + run.start - lo, at + run.end - lo);
                let j0 = mul_div_ceil(i0, cnt, m as u64);
                let j1 = mul_div_ceil(i1, cnt, m as u64);
                if j0 < j1 {
                    f(s + j0..s + j1);
                }
            }
        }
    }

    /// The father: the mother's husband if she was married, their union
    /// isn't void, hasn't separated by the conception, and he was alive at
    /// it. Each fact computed once.
    pub fn father(&self, x: Pid) -> Option<Pid> {
        let bridge = self.bridge(x.cell, x.y);
        let (ym, married, ci, rho) = self.mother_class_slot(x, &bridge)?;
        if !married {
            return None;
        }
        let r = self.crec(x.cell, ym, ci);
        let m = Pid { cell: x.cell, y: ym, i: r.start + rec_inv(r, rho) };
        let conception = self.at_phase(x.cell, x.y, rec_phase(r, rho, x.y)) - GESTATION;
        let (h, start) = self.partner_seat(m)?;
        let dh = self.death(h);
        if dh <= conception.max(start) || self.death(m) <= start {
            return None;
        }
        if self.sep_time(m, start).is_some_and(|s| s <= conception) {
            return None;
        }
        Some(h)
    }

    // ---- search ----

    /// Every death class of a cell cohort and sex.
    pub fn classes(&self, cell: u16, y: i32, female: bool) -> Vec<Class> {
        let c = self.c(cell, y);
        let mut v = Vec::new();
        let base = if female { 0 } else { c.women };
        let s = (!female) as usize;
        v.push(Class { kind: Kind::Young, start: base, n: c.young[s], req: None });
        if female {
            for j in 0..c.nwc as usize {
                v.push(self.wife_class(cell, y, j));
            }
            v.push(self.never_class(cell, y, true));
        } else {
            v.push(self.adult_men(cell, y));
        }
        v
    }

    /// Alive at `t` in one class: (closed-form count, members that need a
    /// per-person check). Absolute classes: the slots after the dead.
    /// Young: certainly alive or dead from the cohort's birth-year bounds,
    /// except a band of slots whose age at death falls in the year window.
    pub fn alive_in(&self, cell: u16, y: i32, female: bool, cl: &Class, t: i64) -> (u64, std::ops::Range<u64>) {
        let th = if cl.req.is_some() { self.adult_dead_w(cell, y, female, t) } else { None };
        self.alive_in_with(y, female, cl, t, th)
    }

    /// [`Self::alive_in`] given the cohort's adult threshold at `t`
    /// ([`Self::adult_dead_w`]), shared by its adult classes.
    fn alive_in_with(&self, y: i32, female: bool, cl: &Class, t: i64, th: Option<f64>) -> (u64, std::ops::Range<u64>) {
        if cl.n == 0 || year_start(y) > t {
            return (0, 0..0);
        }
        if cl.req.is_some() {
            return (cl.n - th.map_or(0, |w| count_from_threshold(cl.n, w)), 0..0);
        }
        // Young: death = birth + age(ρ), birth in [ys, ye). Slots dead even
        // if born at the year's end: age ≤ t − ye; alive even if born at its
        // start: age > t − ys. Both prefixes (the age rises with ρ).
        let (ys, ye) = (year_start(y), year_start(y + 1));
        let upto = |lim: i64| self.young_dead_w(y, female, lim).map_or(0, |w| count_from_threshold(cl.n, w));
        let (dead_sure, alive_sure) = (upto(t - ye), upto(t - ys));
        (cl.n - alive_sure, dead_sure..alive_sure)
    }

    /// A cell cohort's adults of one sex alive at `t` (`y` before `t`'s
    /// year): every adult class against the one shared threshold.
    fn adults_alive(&self, cell: u16, y: i32, female: bool, t: i64) -> u64 {
        let c = self.c(cell, y);
        let th = self.adult_dead_w(cell, y, female, t);
        let alive = |n: u64| n - th.map_or(0, |w| count_from_threshold(n, w));
        if female {
            let mut k = alive(c.women - c.young[0] - c.wives);
            for j in 0..c.nwc as usize {
                k += alive(c.db(j + 1) - c.db(j));
            }
            k
        } else {
            alive(c.men())
        }
    }

    /// A cell cohort's young of one sex at `t` (`y` before `t`'s year): the
    /// surely alive, and the band of slots to check one by one.
    fn young_alive(&self, cell: u16, y: i32, female: bool, t: i64) -> (u64, std::ops::Range<u64>) {
        let cl = self.young_class(cell, y, female);
        self.alive_in_with(y, female, &cl, t, None)
    }

    fn young_class(&self, cell: u16, y: i32, female: bool) -> Class {
        let c = self.c(cell, y);
        Class { kind: Kind::Young, start: if female { 0 } else { c.women }, n: c.young[(!female) as usize], req: None }
    }

    /// Every (cell, cohort) up to the year of `t`.
    fn cell_years(&self, t: i64) -> Vec<(u16, i32)> {
        let ty = procedural_core::stream::year_of(t);
        (0..self.cells())
            .flat_map(|cell| (self.b_min..=ty.min(self.y1)).map(move |y| (cell, y)))
            .collect()
    }

    /// Everyone alive at `t`, bounded without any per-person check:
    /// `(low, high)`, the closed-form count and that plus the infant band
    /// (the young whose age at death is within a year of `t`, and the
    /// birth-year cohort's young who could have died by `t`).
    pub fn alive_bounds(&self, t: i64) -> (u64, u64) {
        let ty = procedural_core::stream::year_of(t);
        self.cell_years(t)
            .par_iter()
            .map(|&(cell, y)| {
                if y == ty {
                    if y < self.p.y0 {
                        return (0, self.c(cell, y).n);
                    }
                    let born = self.born_by_par(cell, y, t);
                    let mut maybe = 0u64;
                    for female in [true, false] {
                        maybe += self.young_dying_slots(y, female, &self.young_class(cell, y, female), t);
                    }
                    return (born.saturating_sub(maybe), born);
                }
                let (mut lo, mut band) = (0u64, 0u64);
                for female in [true, false] {
                    let (n, b) = self.young_alive(cell, y, female, t);
                    lo += n + self.adults_alive(cell, y, female, t);
                    band += b.end - b.start;
                }
                (lo, lo + band)
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    }

    /// Everyone alive at `t`, counted: (closed form, the correction from
    /// people checked one by one, people checked). The birth-year cohort is
    /// its births by `t` (closed form) less its young dead by `t`; the
    /// young whose age at death is within a year of `t` are checked in
    /// parallel chunks (they crowd into the latest cohorts).
    pub fn count_alive(&self, t: i64) -> (u64, i64, u64) {
        let ty = procedural_core::stream::year_of(t);
        // Closed-form parts, and the bands to check: (cell, cohort, sex, class, slots).
        type Band = (u16, i32, bool, Class, std::ops::Range<u64>);
        let parts: Vec<(u64, i64, u64, Vec<Band>)> = self
            .cell_years(t)
            .par_iter()
            .map(|&(cell, y)| {
                if y == ty {
                    if y < self.p.y0 {
                        let k = (0..self.c(cell, y).n).filter(|&i| self.alive_at(Pid { cell, y, i }, t)).count();
                        return (0, k as i64, self.c(cell, y).n, Vec::new());
                    }
                    let born = self.born_by_par(cell, y, t);
                    // Its young who could have died by `t` (mode: dead).
                    let mut bands = Vec::new();
                    for female in [true, false] {
                        let cl = self.young_class(cell, y, female);
                        let k = self.young_dying_slots(y, female, &cl, t);
                        if k > 0 {
                            bands.push((cell, y, female, cl, 0..k));
                        }
                    }
                    return (born, 0, 0, bands);
                }
                let (mut closed, mut bands) = (0u64, Vec::new());
                for female in [true, false] {
                    let (n, band) = self.young_alive(cell, y, female, t);
                    closed += n + self.adults_alive(cell, y, female, t);
                    if band.start < band.end {
                        bands.push((cell, y, female, self.young_class(cell, y, female), band));
                    }
                }
                (closed, 0, 0, bands)
            })
            .collect();
        let mut v = (0u64, 0i64, 0u64);
        let mut chunks = Vec::new();
        for (a, b, c, bands) in parts {
            v.0 += a;
            v.1 += b;
            v.2 += c;
            for (cell, y, female, cl, band) in bands {
                let mut s = band.start;
                while s < band.end {
                    let e = (s + 64).min(band.end);
                    chunks.push((cell, y, female, cl, s..e));
                    s = e;
                }
            }
        }
        let (alive, checked) = chunks
            .par_iter()
            .map(|(cell, y, female, cl, r)| {
                let perm = self.perm(*cell, *y, *female, cl);
                let bridge = self.bridge(*cell, *y);
                let birth_year = *y == ty;
                let mut k = 0i64;
                for rho in r.clone() {
                    // Young: death = birth + age(ρ). Earlier cohorts: alive iff
                    // the age at death reaches past `t`. The birth-year
                    // cohort (counted as born by `t`): less those dead by `t`.
                    let x = Pid { cell: *cell, y: *y, i: cl.start + perm.inv(rho) };
                    let b = self.birth_with(x, &bridge);
                    let a = self.young_secs(*y, *female, cl.n, rho);
                    k += if birth_year { -((b <= t && b + a <= t) as i64) } else { (b <= t && t < b + a) as i64 };
                }
                (k, r.end - r.start)
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        (v.0, v.1 + alive, v.2 + checked)
    }

    /// The young slots of a class whose age at death is at most
    /// `t − year_start(y)`: those who could be dead by `t` if born by it
    /// (a prefix, since the age rises with the slot).
    fn young_dying_slots(&self, y: i32, female: bool, cl: &Class, t: i64) -> u64 {
        if cl.n == 0 {
            return 0;
        }
        self.young_dead_w(y, female, t - year_start(y)).map_or(0, |w| count_from_threshold(cl.n, w))
    }

    /// A cell cohort's members alive at `t`, each passed to `f`: each
    /// class's alive slots through its permutation and the young's band
    /// checked one by one; the birth-year cohort from its births by `t`.
    pub fn alive_in_cohort(&self, cell: u16, y: i32, t: i64, f: &mut impl FnMut(Pid)) {
        let ty = procedural_core::stream::year_of(t);
        if y > ty || !self.exists(y) {
            return;
        }
        if y == ty {
            if y < self.p.y0 {
                for i in 0..self.c(cell, y).n {
                    if self.alive_at(Pid { cell, y, i }, t) {
                        f(Pid { cell, y, i });
                    }
                }
                return;
            }
            let bridge = self.bridge(cell, y);
            let c = self.c(cell, y);
            self.born_by(cell, y, t, |r| {
                for q in r {
                    let x = Pid { cell, y, i: bridge.fwd(q) };
                    let young = if x.i < c.women { x.i < c.young[0] } else { x.i - c.women < c.young[1] };
                    if !young || self.death(x) > t {
                        f(x);
                    }
                }
            });
            return;
        }
        for female in [true, false] {
            let th = self.adult_dead_w(cell, y, female, t);
            for cl in self.classes(cell, y, female) {
                let perm = self.perm(cell, y, female, &cl);
                let (n, band) = self.alive_in_with(y, female, &cl, t, th);
                for r in perm.inv_range(cl.n - n, cl.n) {
                    f(Pid { cell, y, i: cl.start + r });
                }
                for rho in band {
                    let x = Pid { cell, y, i: cl.start + perm.inv(rho) };
                    if self.alive_at(x, t) {
                        f(x);
                    }
                }
            }
        }
    }

    /// Everyone alive at `t`, each passed to `f` (cohorts in parallel).
    pub fn for_each_alive(&self, t: i64, f: impl Fn(Pid) + Sync) {
        let ty = procedural_core::stream::year_of(t).min(self.y1);
        let jobs: Vec<(u16, i32)> = (0..self.cells()).flat_map(|c| (self.b_min..=ty).map(move |y| (c, y))).collect();
        jobs.into_par_iter().for_each(|(cell, y)| self.alive_in_cohort(cell, y, t, &mut |x| f(x)));
    }
}
