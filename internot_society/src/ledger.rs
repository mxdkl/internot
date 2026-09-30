//! The demographic ledger: integer counts both sides of every relation read.
//!
//! Built once per parameter set by a year-by-year cohort projection. It holds
//! only the counts that link blocks:
//! - **block sizes**, which *are* the births to earlier blocks;
//! - **union cells**, the two-sex market's integer matrix per year: women of
//!   block `i` who start a union with men of block `j`, with the same number
//!   recorded on both sides, split by **dissolution class** (the couple's
//!   separation year, or none; R1c);
//! - **births by mother block**, so each child block knows which mothers it
//!   draws from.
//!
//! Deaths are not counted; they are keyed per-person draws (see
//! [`crate::world`]). The ledger's expected never-partnered pools only size
//! the markets, so rounding in them never breaks consistency.
//!
//! A block's members are its **entry cohorts**: the natives (born in-world,
//! or the founders), then the immigrants who arrived in each year (R1 plan,
//! R1b-2). Every cohort has its own pools; a block's unions each year are
//! split over its cohorts exactly.
//!
//! Blocks are `(birth year, lineage region)`. Each year every block's
//! desired unions split between its region's local market and one national
//! market (R1 plan, R1b-1); the markets' integer matrices are merged into
//! one union cell per block, sex and year.
//!
//! Spec §5.2; R1 plan decisions D-R1.1–D-R1.5.

use procedural_core::key::{label, Key};
use procedural_core::partition::{contingency_systematic, SystematicShares};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use std::collections::BTreeMap;

use crate::params::{
    age_gap_weight, arrival_age_weight, couple_arrival_share, death_prob, dissolution_class_pmf,
    first_union_hazard, immigrant_male_share, immigration_rate, national_market_share,
    remarriage_base, remarriage_duration, remarriage_gap_weight, same_sex_gap_weight,
    same_sex_share, status_affinity, Params, Sex, ARRIVAL_GAP_MIN, MAX_AGE, MAX_ARRIVAL_AGE,
    MAX_CLASS, MAX_REMARRIAGE_AGE, MIN_ARRIVAL_AGE, MIN_COUPLE_ARRIVAL_AGE,
};
use crate::plan::{
    apportion, arrival_births, arrival_plans, leaf_births, nonunion_plans, union_age_density,
    PlanLeaf, PlanTables,
};

/// Youngest age at which a union starts (so everyone is at least 15 on the
/// union date).
pub const MIN_UNION_AGE: i32 = 16;
/// Oldest age at which a woman starts a first union.
pub const MAX_UNION_AGE_F: i32 = 65;
/// Oldest age at which a man starts a first union.
pub const MAX_UNION_AGE_M: i32 = 70;

/// A block's union cell for one year, kind and dissolution class: members
/// who start their (first) union that year and whose couples separate in
/// the same year (or never), split by the partner's block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnionCell {
    /// Union year.
    pub year: i32,
    /// Members in the cell.
    pub total: u64,
    /// `(partner block, count)`, in ascending partner-block order.
    pub partners: Vec<(u32, u64)>,
    /// `(cohort, count)`: the members by entry cohort, in cohort order,
    /// nonzero only. Independent of the partner split. Empty for
    /// second-union cells, whose members come from `sources`.
    pub cohorts: Vec<(u16, u64)>,
    /// Second-union cells (R1c): `(divorced source, count)`, the members by
    /// the first-union class-cell they divorced from (an index into the
    /// block's [`Block::div_f`] or [`Block::div_m`]), in source order.
    pub sources: Vec<(u32, u64)>,
    /// Which union cell of the year this is; cells are ordered by `(year,
    /// kind, class)`.
    pub kind: CellKind,
    /// Dissolution class (R1c): `0`, the union lasts until a partner dies;
    /// `k`, the couple separates `k` calendar years after the union year
    /// (after the arrival year, for couples who arrive together). Each
    /// slice's couples are split over classes on the slice, so both sides
    /// record the same classes; kin repair works within a class-cell.
    /// Same-sex cells are class 0 only: their dissolution is a keyed draw.
    pub class: u8,
}

/// The kinds of union cell a block can have in one year, in cell order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellKind {
    /// Opposite-sex unions formed in-world that year.
    InWorld = 0,
    /// Same-sex unions, "left" side: the same-sex market pairs a left and a
    /// right half of each sex's seekers, so the opposite-sex coupling and
    /// kin repair apply unchanged with left in the women's role.
    SameLeft = 1,
    /// Same-sex unions, "right" side (the men's role).
    SameRight = 2,
    /// Couples who arrive together that year, their union begun abroad
    /// (R1b-2 step B). Kept apart from the in-world cell, with its own
    /// slices and plans, so an arriving wife is never paired with a man who
    /// did not arrive with her.
    Arrival = 3,
    /// Opposite-sex unions formed in-world that year in which this cell's
    /// members are in their first union and their partners, divorced, in
    /// their second (R1c).
    FirstWithSecond = 4,
    /// The mirror: this cell's members are divorced and in their second
    /// union, their partners in their first.
    SecondWithFirst = 5,
    /// Both partners divorced and in their second union.
    SecondWithSecond = 6,
}

impl CellKind {
    /// The kind of the partner's cell.
    pub fn partner(self) -> CellKind {
        match self {
            CellKind::SameLeft => CellKind::SameRight,
            CellKind::SameRight => CellKind::SameLeft,
            CellKind::FirstWithSecond => CellKind::SecondWithFirst,
            CellKind::SecondWithFirst => CellKind::FirstWithSecond,
            k => k,
        }
    }

    /// The kind of an in-world opposite-sex union cell for a member who is
    /// (or not) divorced, with a partner who is (or not).
    pub fn of_status(divorced: bool, partner_divorced: bool) -> CellKind {
        match (divorced, partner_divorced) {
            (false, false) => CellKind::InWorld,
            (false, true) => CellKind::FirstWithSecond,
            (true, false) => CellKind::SecondWithFirst,
            (true, true) => CellKind::SecondWithSecond,
        }
    }

    /// True if this cell's members are in their second union.
    pub fn second(self) -> bool {
        matches!(self, CellKind::SecondWithFirst | CellKind::SecondWithSecond)
    }

    /// True if either partner is in a second union.
    pub fn remarriage(self) -> bool {
        matches!(
            self,
            CellKind::FirstWithSecond | CellKind::SecondWithFirst | CellKind::SecondWithSecond
        )
    }

    /// True if the couple feeds the divorced pools when it separates: this
    /// cell's members are in an opposite-sex first union. (Second unions
    /// may dissolve, but there is no third; same-sex couples don't
    /// re-partner in R1c.)
    pub fn first_opposite(self) -> bool {
        matches!(
            self,
            CellKind::InWorld | CellKind::Arrival | CellKind::FirstWithSecond
        )
    }

    /// Number of kinds.
    pub const COUNT: usize = 7;

    /// True for same-sex cells.
    pub fn same_sex(self) -> bool {
        matches!(self, CellKind::SameLeft | CellKind::SameRight)
    }
}

/// An entry cohort of a block: its natives, or the immigrants who arrived
/// in one year.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cohort {
    /// Arrival year; `None` for natives.
    pub arrival: Option<i32>,
    /// Members.
    pub size: u64,
    /// Women among them.
    pub females: u64,
    /// For children who arrive with their parents: births abroad to each
    /// arriving mother block (`union_births`), in mother-block order. Empty
    /// otherwise.
    pub mothers: Vec<MotherShare>,
}

/// Births into a block from one mother block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotherShare {
    /// Mother block index.
    pub mother: u32,
    /// Births from union plans.
    pub union_births: u64,
    /// Births from non-union plans.
    pub nonunion_births: u64,
}

/// One birth cohort: a lineage region's births in one year.
#[derive(Clone, Debug)]
pub struct Block {
    /// Birth year.
    pub year: i32,
    /// Lineage region (index into [`Params::regions`]).
    pub region: u16,
    /// Founders were alive at `y0` and have no in-world parents.
    pub founder: bool,
    /// Members, over all entry cohorts.
    pub size: u64,
    /// Women, over all entry cohorts.
    pub females: u64,
    /// Entry cohorts: natives first (possibly empty), then arrivals in
    /// arrival-year order. Raw ids follow this order.
    pub cohorts: Vec<Cohort>,
    /// Union cells of the block's women, in year order.
    pub union_f: Vec<UnionCell>,
    /// Union cells of the block's men, in year order.
    pub union_m: Vec<UnionCell>,
    /// Where the block's natives come from, in mother-block order.
    pub mothers: Vec<MotherShare>,
    /// Divorced sources (R1c): the block's women's and men's first-union
    /// class-cells that separate, in the order they were recorded.
    pub div_f: Vec<DivSource>,
    pub div_m: Vec<DivSource>,
}

/// A divorced source (R1c): the members of one first-union class-cell of
/// a block, who separate in the class's year, and where they go next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DivSource {
    /// The class-cell: its year, kind and class. The members separate in
    /// `year + class` and may start a second union from the next year.
    pub year: i32,
    pub kind: CellKind,
    pub class: u8,
    /// Members.
    pub members: u64,
    /// Remarriage parts: `(second-union cell year, kind, class, count)`,
    /// in the order they were recorded; the rest never re-partner.
    pub parts: Vec<(i32, CellKind, u8, u64)>,
}

impl DivSource {
    /// The calendar year the members separate.
    pub fn divorce_year(&self) -> i32 {
        self.year + self.class as i32
    }
}

/// The ledger for one world.
#[derive(Clone, Debug)]
pub struct Ledger {
    pub params: Params,
    /// Birth year of block 0.
    pub first_year: i32,
    /// Lineage regions; block `(year, r)` has index
    /// `(year - first_year) * regions + r`.
    pub regions: usize,
    pub blocks: Vec<Block>,
    /// `base[b]` is block `b`'s first id; `base[len]` is the id count.
    pub base: Vec<u64>,
    /// Root key of the plan partitions, shared with the world so both
    /// derive the same leaves (see [`plan_key`]).
    pub plan_root: Key,
    /// Couples moved to a neighbouring dissolution class because they were
    /// alone on both sides of theirs (see [`deisolate`]).
    pub class_moves: u64,
    /// [`union_age_density`] per arrival year (index `year - y0`), shared
    /// with the world so arrival plans cost no hazard evaluations.
    pub arrival_density: Vec<Vec<f64>>,
    /// Plan shares for every union year, shared with the world.
    pub plan_tables: PlanTables,
}

impl Ledger {
    /// The union-age density of arrival year `t` (see [`union_age_density`]).
    pub fn arrival_density(&self, t: i32) -> &[f64] {
        &self.arrival_density[(t - self.params.y0) as usize]
    }
}

const TAG_ROUND: u64 = label("ledger/round");
const TAG_CLASS: u64 = label("ledger/class");
const TAG_ARRIVAL_CLASS: u64 = label("ledger/arrival-class");
const TAG_PLANS: u64 = label("ledger/plans");
const TAG_CONTINGENCY: u64 = label("ledger/contingency");
const TAG_SETTLE: u64 = label("ledger/settle");

/// Births by year and mother block: `(union, non-union)`, year-major over
/// every block the ledger will have.
struct Births {
    first_year: i32,
    blocks: usize,
    counts: Vec<(u64, u64)>,
}

impl Births {
    fn new(first_year: i32, years: usize, blocks: usize) -> Self {
        Self {
            first_year,
            blocks,
            counts: vec![(0, 0); years * blocks],
        }
    }

    fn at(&mut self, year: i32, mother: u32) -> &mut (u64, u64) {
        let y = (year - self.first_year) as usize;
        &mut self.counts[y * self.blocks + mother as usize]
    }

    /// Year `year`'s counts, indexed by mother block.
    fn year(&self, year: i32) -> impl Iterator<Item = (usize, &(u64, u64))> {
        let y = (year - self.first_year) as usize;
        self.counts[y * self.blocks..(y + 1) * self.blocks]
            .iter()
            .enumerate()
    }
}

/// The plan partitions' root key and shares.
#[derive(Clone, Copy)]
struct Plans<'a> {
    root: Key,
    tables: &'a PlanTables,
}

/// Where a class-cell's members come from: entry cohorts (first unions) or
/// divorced sources (second unions, R1c).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Members {
    Cohorts,
    Sources,
}

/// A block's class-cells for one year and kind, from its couples by
/// `(class, partner block, count)` and its members by entry cohort or
/// divorced source (`(index, count)`): one cell per class, each with its
/// members split over the cohorts or sources by a two-margin systematic
/// table (exact both ways, and unbiased).
fn class_cells(
    year: i32,
    kind: CellKind,
    partners: &mut [(u8, u32, u64)],
    members: &[(u32, u64)],
    from: Members,
    key: Key,
) -> Vec<UnionCell> {
    partners.sort_unstable();
    let mut by_class: Vec<(u8, Vec<(u32, u64)>)> = Vec::new();
    for &mut (class, block, n) in partners {
        match by_class.last_mut() {
            Some((c, list)) if *c == class => list.push((block, n)),
            _ => by_class.push((class, vec![(block, n)])),
        }
    }
    let sizes: Vec<u64> = by_class
        .iter()
        .map(|(_, l)| l.iter().map(|p| p.1).sum())
        .collect();
    let counts: Vec<u64> = members.iter().map(|c| c.1).collect();
    let mut table = vec![0u64; counts.len() * sizes.len()];
    contingency_systematic(&counts, &sizes, key, &mut table);
    by_class
        .into_iter()
        .enumerate()
        .map(|(k, (class, partners))| {
            let split: Vec<(u32, u64)> = members
                .iter()
                .enumerate()
                .map(|(h, &(m, _))| (m, table[h * sizes.len() + k]))
                .filter(|&(_, n)| n > 0)
                .collect();
            let (cohorts, sources) = match from {
                Members::Cohorts => (
                    split.iter().map(|&(c, n)| (c as u16, n)).collect(),
                    Vec::new(),
                ),
                Members::Sources => (Vec::new(), split),
            };
            UnionCell {
                year,
                total: sizes[k],
                partners,
                cohorts,
                sources,
                kind,
                class,
            }
        })
        .collect()
}

/// Key of one sub-cell's plan partition: the women of `block` who start a
/// union of `kind` in `year` and whose couples have dissolution `class`.
pub fn plan_key(root: Key, block: u32, year: i32, kind: CellKind, class: u8) -> Key {
    root.with3(
        block as u64,
        year as i64 as u64,
        ((kind as u64) << 8) | class as u64,
    )
}

/// The calendar-year offset of a class's separation from the plan's union
/// year (`None`: class 0, no separation). Arrival classes count from
/// arrival, which [`crate::plan::arrival_births`] handles.
pub(crate) fn cutoff(class: u8) -> Option<i32> {
    (class > 0).then_some(class as i32)
}

/// Period survival `l(age)` at `year` for founders' starting sizes.
fn period_survival(sex: Sex, age: i32, year: i32) -> f64 {
    (0..age.max(0)).fold(1.0, |l, a| l * (1.0 - death_prob(sex, a as u32, year)))
}

/// Probability of never having partnered by `age` under `year`'s hazards.
fn never_partnered_share(sex: Sex, age: i32, year: i32) -> f64 {
    (0..age.max(0)).fold(1.0, |s, a| {
        s * (1.0 - first_union_hazard(sex, a as u32, year))
    })
}

/// Iterative proportional fitting of `k` (rows × cols) to row and column
/// margins. Zero margins give zero rows/columns. Stops once every row sum
/// is within `1e-10` (relative) of its margin after a column pass, or after
/// 60 passes; both are deterministic, so the result is too.
fn ipf(k: &mut [f64], rows: usize, cols: usize, r: &[f64], c: &[f64]) {
    // Row-major throughout: column sums accumulate row by row, in the same
    // order as a column walk, and each pass fuses a scaling with the next
    // sums, so the results are bit-identical to separate row and column
    // passes.
    let mut colsum = vec![0.0; cols];
    let mut fac = vec![0.0; cols];
    let mut sums = vec![0.0; rows];
    scale_cols_and_sum_rows(k, cols, None, &mut sums);
    for it in 0..=60 {
        let converged = it > 0
            && sums
                .iter()
                .zip(r)
                .all(|(&s, &ri)| (s - ri).abs() <= 1e-10 * ri.max(1.0));
        if converged || it == 60 {
            break;
        }
        colsum.fill(0.0);
        for (row, (&s, &ri)) in k.chunks_exact_mut(cols).zip(sums.iter().zip(r)) {
            let f = if s > 0.0 { ri / s } else { 0.0 };
            for (x, acc) in row.iter_mut().zip(colsum.iter_mut()) {
                *x *= f;
                *acc += *x;
            }
        }
        for ((f, &s), &cj) in fac.iter_mut().zip(&colsum).zip(c) {
            *f = if s > 0.0 { cj / s } else { 0.0 };
        }
        scale_cols_and_sum_rows(k, cols, Some(&fac), &mut sums);
    }
}

/// Scale each column of a row-major `rows × cols` matrix by `fac` (if
/// given), then write each row's sum to `out`, added left to right exactly
/// as `iter().sum()` does. A float sum is a chain of dependent adds, so
/// eight rows are summed side by side to overlap their chains; each row's
/// own order, and so its result, is unchanged.
fn scale_cols_and_sum_rows(k: &mut [f64], cols: usize, fac: Option<&[f64]>, out: &mut [f64]) {
    const LANES: usize = 8;
    let mut i = 0;
    let mut chunks = out.chunks_exact_mut(LANES);
    for chunk in &mut chunks {
        let block = &mut k[i * cols..(i + LANES) * cols];
        let mut acc = [0.0f64; LANES];
        for j in 0..cols {
            let f = fac.map(|f| f[j]);
            for (l, a) in acc.iter_mut().enumerate() {
                let x = &mut block[l * cols + j];
                if let Some(f) = f {
                    *x *= f;
                }
                *a += *x;
            }
        }
        chunk.copy_from_slice(&acc);
        i += LANES;
    }
    for o in chunks.into_remainder() {
        let row = &mut k[i * cols..(i + 1) * cols];
        if let Some(fac) = fac {
            row.iter_mut().zip(fac).for_each(|(x, &f)| *x *= f);
        }
        *o = row.iter().sum();
        i += 1;
    }
}

/// Mutable projection state for one entry cohort, by sex (index
/// `Sex as usize`): expected never-partnered and alive, members, and members
/// already in a union.
#[derive(Clone, Copy)]
struct CohortPool {
    never: [f64; 2],
    alive: [f64; 2],
    size: [u64; 2],
    used: [u64; 2],
}

impl CohortPool {
    fn new(f: u64, m: u64, never: [f64; 2]) -> Self {
        Self {
            never,
            alive: [f as f64, m as f64],
            size: [f, m],
            used: [0, 0],
        }
    }
}

/// Mutable projection state for one block's divorced source (R1c): its
/// index in the block's list, its expected members alive and not
/// re-partnered, scaled by the pool's survival factor (see [`DivPools`]),
/// and its members not yet re-partnered.
#[derive(Clone, Copy)]
struct DivPool {
    source: u32,
    scaled: f64,
    left: u64,
}

/// One divorce year of a block's divorced pool: its sources (stored here,
/// so a sweep over them reads one array), the sum of their scaled expected
/// members, and their members not re-partnered.
#[derive(Clone)]
struct DivYear {
    year: i32,
    scaled: f64,
    unused: u64,
    sources: Vec<DivPool>,
}

/// A block's divorced pool for one sex (R1c). All its members age and die
/// alike, so expected survivors are stored scaled by one shared survival
/// factor: mortality is one multiplication, and the hazard (which depends on
/// years since divorce) weighs divorce years, not sources.
#[derive(Clone)]
struct DivPools {
    /// Sources added so far (the next source's index).
    count: u32,
    /// Divorce years with sources that can still re-partner, ascending.
    years: Vec<DivYear>,
    /// Survival factor: expected = scaled × factor.
    factor: f64,
}

impl Default for DivPools {
    fn default() -> Self {
        Self {
            count: 0,
            years: Vec::new(),
            factor: 1.0,
        }
    }
}

impl DivPools {
    /// Add a source of `members` who separate in `divorce_year`.
    fn add(&mut self, divorce_year: i32, members: u64) {
        let scaled = members as f64 / self.factor;
        let source = self.count;
        self.count += 1;
        let k = self.years.partition_point(|y| y.year < divorce_year);
        if self.years.get(k).map_or(true, |y| y.year != divorce_year) {
            self.years.insert(
                k,
                DivYear {
                    year: divorce_year,
                    scaled: 0.0,
                    unused: 0,
                    sources: Vec::new(),
                },
            );
        }
        let y = &mut self.years[k];
        y.scaled += scaled;
        y.unused += members;
        y.sources.push(DivPool {
            source,
            scaled,
            left: members,
        });
    }

    /// Divorce years available for a second union in year `t`.
    fn available(&self, t: i32) -> &[DivYear] {
        &self.years[..self.years.partition_point(|y| y.year < t)]
    }

    /// Take `k` members from source `i` (its position) of divorce year `yi`.
    fn take(&mut self, yi: usize, i: usize, k: u64) {
        let y = &mut self.years[yi];
        let d = &mut y.sources[i];
        let dk = (k as f64 / self.factor).min(d.scaled);
        d.left -= k;
        d.scaled -= dk;
        y.scaled -= dk;
        y.unused -= k;
    }

    /// Retire the sources of divorce year `yi` that can no longer
    /// re-partner.
    fn retire_sources(&mut self, yi: usize) {
        let factor = self.factor;
        self.years[yi]
            .sources
            .retain(|d| d.left > 0 && d.scaled * factor > RETIRED);
    }

    /// Retire the divorce years that can no longer re-partner.
    fn retire_years(&mut self) {
        let factor = self.factor;
        self.years
            .retain(|y| !y.sources.is_empty() && y.unused > 0 && y.scaled * factor > RETIRED);
    }
}

/// Mutable projection state for one block: its cohorts and divorced
/// sources, plus the unions taken this year, by cell kind and sex, not yet
/// split over the cohorts or sources.
struct Pool {
    cohorts: Vec<CohortPool>,
    div: [DivPools; 2],
    taken: [[u64; 2]; CellKind::COUNT],
}

/// Below this many expected survivors a divorced source is retired from
/// the markets (its weight and cap contribution are nil).
const RETIRED: f64 = 1e-3;

/// Kinds whose members come from the never-partnered pool.
const NEVER_KINDS: [CellKind; 4] = [
    CellKind::InWorld,
    CellKind::SameLeft,
    CellKind::SameRight,
    CellKind::FirstWithSecond,
];
/// Kinds whose members come from the divorced pool.
const DIVORCED_KINDS: [CellKind; 2] = [CellKind::SecondWithFirst, CellKind::SecondWithSecond];

impl Pool {
    fn one(c: CohortPool) -> Self {
        Self {
            cohorts: vec![c],
            div: [DivPools::default(), DivPools::default()],
            taken: [[0; 2]; CellKind::COUNT],
        }
    }

    fn taken_of(&self, kinds: &[CellKind], s: Sex) -> u64 {
        kinds
            .iter()
            .map(|&k| self.taken[k as usize][s as usize])
            .sum()
    }

    /// Expected never-partnered members left this year.
    fn never(&self, s: Sex) -> f64 {
        let n: f64 = self
            .cohorts
            .iter()
            .map(|c| c.never[s as usize].max(0.0))
            .sum();
        n - self.taken_of(&NEVER_KINDS, s) as f64
    }

    /// Members not yet in a union, net of this year's takings.
    fn unused(&self, s: Sex) -> u64 {
        let n: u64 = self
            .cohorts
            .iter()
            .map(|c| c.size[s as usize] - c.used[s as usize])
            .sum();
        n - self.taken_of(&NEVER_KINDS, s)
    }

    /// Desired second unions this year of the divorced, aged `age`.
    fn div_want(&self, s: Sex, age: i32, t: i32) -> f64 {
        let base = remarriage_base(s, age, t);
        if base == 0.0 {
            return 0.0;
        }
        let d = &self.div[s as usize];
        d.available(t)
            .iter()
            .map(|y| remarriage_duration(t - y.year) * y.scaled.max(0.0))
            .sum::<f64>()
            * d.factor
            * base
    }

    /// Most second unions the divorced can take this year: expected
    /// available survivors, and available members, net of this year's
    /// takings.
    fn div_cap(&self, s: Sex, t: i32) -> u64 {
        let d = &self.div[s as usize];
        let (mut e, mut n) = (0.0, 0u64);
        for y in d.available(t) {
            e += y.scaled.max(0.0);
            n += y.unused;
        }
        let taken = self.taken_of(&DIVORCED_KINDS, s);
        ((e * d.factor).floor() as u64).min(n).saturating_sub(taken)
    }

    /// Most unions of `divorced` or never-partnered seekers this year.
    fn cap(&self, s: Sex, divorced: bool, t: i32) -> u64 {
        if divorced {
            self.div_cap(s, t)
        } else {
            (self.never(s).floor().max(0.0) as u64).min(self.unused(s))
        }
    }

    /// Split this year's takings of one never-partnered kind over the
    /// cohorts: by never-partnered pool, capped by each cohort's unused
    /// members. Returns `(cohort, count)`.
    fn settle(&mut self, kind: CellKind, s: Sex) -> Vec<(u32, u64)> {
        let si = s as usize;
        let n = std::mem::take(&mut self.taken[kind as usize][si]);
        if n == 0 {
            return Vec::new();
        }
        let weights: Vec<f64> = self.cohorts.iter().map(|c| c.never[si].max(0.0)).collect();
        let caps: Vec<u64> = self
            .cohorts
            .iter()
            .map(|c| c.size[si] - c.used[si])
            .collect();
        let split = apportion_capped(n, &weights, &caps);
        let mut out = Vec::new();
        for (ci, (c, &k)) in self.cohorts.iter_mut().zip(&split).enumerate() {
            if k > 0 {
                c.used[si] += k;
                c.never[si] -= k as f64;
                out.push((ci as u32, k));
            }
        }
        out
    }

    /// Split this year's takings of one divorced kind over the available
    /// sources: first over divorce years by want (hazard times expected
    /// survivors), then within each year over its sources by expected
    /// survivors, capped by members not yet re-partnered. Each level is a
    /// keyed systematic sweep ([`sweep`]): exact, unbiased, linear and
    /// allocation-free. Returns `(source, count)` in source order.
    fn settle_div(&mut self, kind: CellKind, s: Sex, t: i32, key: Key) -> Vec<(u32, u64)> {
        let si = s as usize;
        let n = std::mem::take(&mut self.taken[kind as usize][si]);
        if n == 0 {
            return Vec::new();
        }
        let d = &mut self.div[si];
        let ny = d.available(t).len();
        let mut by_year = vec![0u64; ny];
        {
            let years = &d.years[..ny];
            sweep(
                n,
                ny,
                |yi| {
                    let y = &years[yi];
                    (
                        remarriage_duration(t - y.year) * y.scaled.max(0.0),
                        y.unused,
                    )
                },
                key.unit(),
                |yi, k| by_year[yi] += k,
            );
        }
        let mut out = Vec::new();
        for (yi, &ky) in by_year.iter().enumerate() {
            if ky == 0 {
                continue;
            }
            let mut got: Vec<(usize, u64)> = Vec::new();
            {
                let srcs = &d.years[yi].sources;
                sweep(
                    ky,
                    srcs.len(),
                    |k| (srcs[k].scaled.max(0.0), srcs[k].left),
                    key.with(yi as u64 + 1).unit(),
                    |k, c| got.push((k, c)),
                );
            }
            for &(i, k) in &got {
                out.push((d.years[yi].sources[i].source, k));
                d.take(yi, i, k);
            }
            d.retire_sources(yi);
        }
        d.retire_years();
        out.sort_unstable();
        // A source can be hit in both of a sweep's passes.
        out.dedup_by(|b, a| {
            let same = a.0 == b.0;
            if same {
                a.1 += b.1;
            }
            same
        });
        out
    }
}

/// Keyed systematic split of `n` over `len` items by weight, capped: item
/// `k` is `item(k) = (weight, cap)`; `n` points spaced `W / n` apart from
/// offset `u` fall into the items' weight intervals. An item over its cap
/// passes the excess on; a second pass gives what remains to items with room,
/// in order. `give(k, count)` receives the counts (an item can get two calls).
/// Requires `n` at most the caps' sum. With no positive weight, the caps are
/// the weights.
fn sweep(
    n: u64,
    len: usize,
    item: impl Fn(usize) -> (f64, u64),
    u: f64,
    mut give: impl FnMut(usize, u64),
) {
    let total: f64 = (0..len).map(|k| item(k).0).sum();
    let by_cap = total <= 0.0;
    let weight = |k: usize| {
        let (w, c) = item(k);
        if by_cap {
            c as f64
        } else {
            w
        }
    };
    let total = if by_cap {
        (0..len).map(|k| item(k).1 as f64).sum()
    } else {
        total
    };
    let step = total / n as f64;
    let (mut next, mut cum, mut placed, mut carry) = (u * step, 0.0, 0u64, 0u64);
    let mut given = vec![0u64; 0];
    let mut room_left = false;
    for k in 0..len {
        cum += weight(k);
        let mut c = carry;
        while placed < n && next < cum {
            c += 1;
            placed += 1;
            next += step;
        }
        let cap = item(k).1;
        let g = c.min(cap);
        carry = c - g;
        if g > 0 {
            give(k, g);
        }
        room_left |= g < cap;
        given.push(g);
    }
    // Points lost to rounding at the end, and excess carried past the last
    // item: to items with room, in order.
    let mut rest = carry + (n - placed);
    if rest > 0 && room_left {
        for k in 0..len {
            if rest == 0 {
                break;
            }
            let room = item(k).1 - given[k];
            let g = room.min(rest);
            if g > 0 {
                give(k, g);
                rest -= g;
            }
        }
    }
    assert_eq!(rest, 0, "more takings than members");
}

/// Register a first-union class-cell that separates as a divorced source
/// of its block (R1c): the ledger's list, and the projection's pool.
fn add_source(block: &mut Block, pool: &mut Pool, s: Sex, cell: &UnionCell) {
    if cell.class == 0 || !cell.kind.first_opposite() {
        return;
    }
    let src = DivSource {
        year: cell.year,
        kind: cell.kind,
        class: cell.class,
        members: cell.total,
        parts: Vec::new(),
    };
    pool.div[s as usize].add(src.divorce_year(), cell.total);
    match s {
        Sex::Female => block.div_f.push(src),
        Sex::Male => block.div_m.push(src),
    }
}

/// Apportion `n` by `weights` (largest remainder) without exceeding `caps`:
/// cohorts that would exceed their cap are fixed at it and the rest is
/// re-apportioned among the others; when their weights run out, by spare
/// capacity. `n` must not exceed the caps' sum.
fn apportion_capped(n: u64, weights: &[f64], caps: &[u64]) -> Vec<u64> {
    assert!(n <= caps.iter().sum::<u64>(), "more unions than members");
    let mut out = vec![0u64; weights.len()];
    let mut open: Vec<bool> = caps.iter().map(|&c| c > 0).collect();
    let mut left = n;
    while left > 0 {
        let idx: Vec<usize> = (0..weights.len()).filter(|&i| open[i]).collect();
        let mut w: Vec<f64> = idx.iter().map(|&i| weights[i].max(0.0)).collect();
        if w.iter().all(|&x| x <= 0.0) {
            w = idx.iter().map(|&i| (caps[i] - out[i]) as f64).collect();
        }
        let add = apportion(left, &w);
        let over: Vec<usize> = idx
            .iter()
            .zip(&add)
            .filter(|&(&i, &a)| a > caps[i] - out[i])
            .map(|(&i, _)| i)
            .collect();
        if over.is_empty() {
            for (&i, &a) in idx.iter().zip(&add) {
                out[i] += a;
            }
            left = 0;
        } else {
            // Fill the overflowing cohorts to their caps and re-apportion
            // the rest; each round closes at least one cohort.
            for i in over {
                left -= caps[i] - out[i];
                out[i] = caps[i];
                open[i] = false;
            }
        }
    }
    out
}

impl Ledger {
    /// Build the ledger. `seed` only keys the integer rounding of market
    /// matrices, so the same parameters and seed always give the same world.
    pub fn build(params: Params, seed: u64) -> Self {
        let key = Key::from_seed(seed).with(TAG_ROUND);
        let plan_root = Key::from_seed(seed).with(TAG_PLANS);
        // Union years: arriving couples' unions began up to their age at
        // arrival before it; every other union starts in y0..=y1.
        let plan_tables = PlanTables::new(params.y0 - MAX_ARRIVAL_AGE, params.y1);
        let plans = Plans {
            root: plan_root,
            tables: &plan_tables,
        };
        let mut class_moves = 0u64;
        let arrival_density: Vec<Vec<f64>> = (params.y0..=params.y1)
            .map(|t| union_age_density(t, MAX_ARRIVAL_AGE))
            .collect();
        let regions = params.region_count();
        assert!(regions >= 1, "a world needs at least one region");
        let first_year = params.y0 - params.founder_max_age;
        let years = (params.y1 - first_year + 1) as usize;
        let male_share = params.male_share_at_birth;
        let mut blocks: Vec<Block> = Vec::with_capacity(years * regions);
        let mut pools: Vec<Pool> = Vec::with_capacity(years * regions);
        let mut births = Births::new(first_year, years, years * regions);
        let weight_sum: f64 = params.regions.iter().map(|r| r.founder_weight).sum();

        // Founders: survivors at y0 of a stable population, split by region.
        for y in first_year..=params.y0 {
            let age = params.y0 - y;
            for (r, region) in params.regions.iter().enumerate() {
                let born = params.founder_births
                    * procedural_core::dmath::exp(-params.founder_growth * age as f64)
                    * region.founder_weight
                    / weight_sum;
                let f = (born * (1.0 - male_share) * period_survival(Sex::Female, age, params.y0))
                    .round() as u64;
                let m =
                    (born * male_share * period_survival(Sex::Male, age, params.y0)).round() as u64;
                let idx = blocks.len() as u32;
                blocks.push(Block {
                    year: y,
                    region: r as u16,
                    founder: true,
                    size: f + m,
                    females: f,
                    cohorts: vec![Cohort {
                        arrival: None,
                        size: f + m,
                        females: f,
                        mothers: Vec::new(),
                    }],
                    union_f: Vec::new(),
                    union_m: Vec::new(),
                    mothers: Vec::new(),
                    div_f: Vec::new(),
                    div_m: Vec::new(),
                });
                pools.push(Pool::one(CohortPool::new(
                    f,
                    m,
                    [
                        f as f64 * never_partnered_share(Sex::Female, age, params.y0),
                        m as f64 * never_partnered_share(Sex::Male, age, params.y0),
                    ],
                )));
                add_nonunion_births(&mut births, idx, f, y, params.y0 + 1, params.y1);
            }
        }

        // Founder couples already partnered at y0.
        {
            let y0 = params.y0;
            let wants = |b: usize, sex: Sex, max_age: i32| -> Option<f64> {
                (MIN_UNION_AGE..=max_age + 30)
                    .contains(&(y0 - blocks[b].year))
                    .then(|| match sex {
                        Sex::Female => blocks[b].females as f64 - pools[b].never(sex),
                        Sex::Male => {
                            (blocks[b].size - blocks[b].females) as f64 - pools[b].never(sex)
                        }
                    })
            };
            let f: Vec<Option<f64>> = (0..blocks.len())
                .map(|b| wants(b, Sex::Female, MAX_UNION_AGE_F))
                .collect();
            let m: Vec<Option<f64>> = (0..blocks.len())
                .map(|b| wants(b, Sex::Male, MAX_UNION_AGE_M))
                .collect();
            let none = vec![None; blocks.len()];
            clear_year(
                y0,
                params.same_sex_boost,
                (&f, &m),
                (&none, &none),
                regions,
                &mut blocks,
                &mut pools,
                &mut births,
                params.y1,
                key,
                plans,
                &mut class_moves,
            );
        }

        // Expected living, carried from year to year for the inflow's base.
        let mut alive: f64 = pools
            .iter()
            .flat_map(|p| &p.cohorts)
            .map(|c| c.alive[0] + c.alive[1])
            .sum();
        for t in params.y0 + 1..=params.y1 {
            // 1. The cohorts born this year: exactly the births recorded for
            //    them, each in its mother's region.
            let mut by_region: Vec<Vec<MotherShare>> = vec![Vec::new(); regions];
            for (mother, &(u, nu)) in births.year(t) {
                if u + nu == 0 {
                    continue;
                }
                let mother = mother as u32;
                by_region[blocks[mother as usize].region as usize].push(MotherShare {
                    mother,
                    union_births: u,
                    nonunion_births: nu,
                });
            }
            for (r, mut mothers) in by_region.into_iter().enumerate() {
                mothers.sort_by_key(|m| m.mother);
                let size: u64 = mothers
                    .iter()
                    .map(|m| m.union_births + m.nonunion_births)
                    .sum();
                let females = crate::plan::apportion(size, &[1.0 - male_share, male_share])[0];
                let idx = blocks.len() as u32;
                blocks.push(Block {
                    year: t,
                    region: r as u16,
                    founder: false,
                    size,
                    females,
                    cohorts: vec![Cohort {
                        arrival: None,
                        size,
                        females,
                        mothers: Vec::new(),
                    }],
                    union_f: Vec::new(),
                    union_m: Vec::new(),
                    mothers,
                    div_f: Vec::new(),
                    div_m: Vec::new(),
                });
                pools.push(Pool::one(CohortPool::new(
                    females,
                    size - females,
                    [females as f64, (size - females) as f64],
                )));
                add_nonunion_births(&mut births, idx, females, t, t + 1, params.y1);
                alive += size as f64;
            }

            // 2. This year's union markets.
            let want = |b: usize, sex: Sex, max_age: i32| -> Option<f64> {
                let age = t - blocks[b].year;
                if !(MIN_UNION_AGE..=max_age).contains(&age) {
                    return None;
                }
                let w = first_union_hazard(sex, age as u32, t) * pools[b].never(sex).max(0.0);
                (w > 0.0).then_some(w)
            };
            let f: Vec<Option<f64>> = (0..blocks.len())
                .map(|b| want(b, Sex::Female, MAX_UNION_AGE_F))
                .collect();
            let m: Vec<Option<f64>> = (0..blocks.len())
                .map(|b| want(b, Sex::Male, MAX_UNION_AGE_M))
                .collect();
            // The divorced's desired second unions (R1c).
            let div_want = |b: usize, sex: Sex| -> Option<f64> {
                let age = t - blocks[b].year;
                if !(MIN_UNION_AGE..=MAX_REMARRIAGE_AGE).contains(&age) {
                    return None;
                }
                let w = pools[b].div_want(sex, age, t);
                (w > 0.0).then_some(w)
            };
            let df: Vec<Option<f64>> = (0..blocks.len())
                .map(|b| div_want(b, Sex::Female))
                .collect();
            let dm: Vec<Option<f64>> = (0..blocks.len()).map(|b| div_want(b, Sex::Male)).collect();
            clear_year(
                t,
                params.same_sex_boost,
                (&f, &m),
                (&df, &dm),
                regions,
                &mut blocks,
                &mut pools,
                &mut births,
                params.y1,
                key,
                plans,
                &mut class_moves,
            );

            // 3. Immigrants arriving this year (R1b-2): singles, couples who
            //    arrive together, and those couples' children born abroad.
            //    Nobody arriving joins an in-world market before next year.
            arrive(
                t,
                alive,
                &params,
                &mut blocks,
                &mut pools,
                &mut births,
                first_year,
                regions,
                (key, plans),
                &arrival_density[(t - params.y0) as usize],
            );
            // This year's cells were recorded market by market; order them
            // by (kind, class) after the earlier years' (every lookup
            // searches cells by (year, kind, class)).
            blocks.par_iter_mut().for_each(|b| {
                for cells in [&mut b.union_f, &mut b.union_m] {
                    let from = cells.partition_point(|c| c.year < t);
                    cells[from..].sort_by_key(|c| (c.kind, c.class));
                }
            });

            // 4. Mortality over the year; the survivors are next year's base.
            alive = 0.0;
            for (b, pool) in pools.iter_mut().enumerate() {
                let age = t - blocks[b].year;
                if age < 0 || age as u32 > MAX_AGE {
                    continue;
                }
                let q = [
                    death_prob(Sex::Female, age as u32, t),
                    death_prob(Sex::Male, age as u32, t),
                ];
                for c in &mut pool.cohorts {
                    for ((never, living), q) in c.never.iter_mut().zip(&mut c.alive).zip(q) {
                        *never *= 1.0 - q;
                        *living *= 1.0 - q;
                        alive += *living;
                    }
                }
                for (d, q) in pool.div.iter_mut().zip(q) {
                    if age > MAX_REMARRIAGE_AGE {
                        // Too old to re-partner: retire every source.
                        d.years.clear();
                        continue;
                    }
                    d.factor *= 1.0 - q;
                    d.retire_years();
                }
            }
        }

        let mut base = Vec::with_capacity(blocks.len() + 1);
        let mut acc = 0u64;
        for b in &blocks {
            base.push(acc);
            acc += b.size;
        }
        base.push(acc);
        Self {
            params,
            first_year,
            regions,
            blocks,
            base,
            plan_root,
            class_moves,
            arrival_density,
            plan_tables,
        }
    }

    /// Total ids.
    pub fn population(&self) -> u64 {
        *self.base.last().unwrap()
    }

    /// Block index for a birth year and region.
    pub fn block_of(&self, year: i32, region: u16) -> Option<u32> {
        let i = year - self.first_year;
        let n_years = (self.blocks.len() / self.regions) as i32;
        ((0..n_years).contains(&i) && (region as usize) < self.regions)
            .then(|| (i as usize * self.regions + region as usize) as u32)
    }

    /// Block indices of every region's cohort born in `year`.
    pub fn blocks_of_year(&self, year: i32) -> std::ops::Range<u32> {
        match self.block_of(year, 0) {
            Some(b) => b..b + self.regions as u32,
            None => 0..0,
        }
    }
}

/// Immigrants arriving in year `t`: per region, `immigration_rate(t)` of
/// the expected living `alive` (this year's births included), a share [`couple_arrival_share`] of them in
/// couples. Couples get arrival union cells and arrival plans; the plans'
/// births abroad are the children who arrive with them, each child's
/// cohort recording its mothers (R1 plan, R1b-2 step B).
#[allow(clippy::too_many_arguments)]
fn arrive(
    t: i32,
    alive: f64,
    params: &Params,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut Births,
    first_year: i32,
    regions: usize,
    (key, plans): (Key, Plans),
    dens: &[f64],
) {
    let arriving = (immigration_rate(t) * alive).round() as u64;
    let region_w: Vec<f64> = params.regions.iter().map(|r| r.immigrant_weight).collect();
    let ages: Vec<i32> = (MIN_ARRIVAL_AGE..=MAX_ARRIVAL_AGE).collect();
    let age_w: Vec<f64> = ages.iter().map(|&a| arrival_age_weight(a)).collect();
    let men_share = immigrant_male_share(t);
    let block_of = |year: i32, r: usize| ((year - first_year) as usize) * regions + r;
    // Share ever partnered by each age under this year's schedule, per sex.
    let ever: [Vec<f64>; 2] = [Sex::Female, Sex::Male].map(|sex| {
        let mut never = 1.0;
        (0..=MAX_ARRIVAL_AGE)
            .map(|a| {
                let e = 1.0 - never;
                never *= 1.0 - first_union_hazard(sex, a as u32, t);
                e
            })
            .collect()
    });
    let partnered = |sex: Sex, age: i32| ever[sex as usize][age as usize];
    for (r, &nr) in apportion(arriving, &region_w).iter().enumerate() {
        // Couples: wives by age, then husbands by the age-gap kernel, never
        // more than |ARRIVAL_GAP_MIN| years younger.
        let couples = (couple_arrival_share(t) * nr as f64 / 2.0).round() as u64;
        let singles = nr - 2 * couples;
        let wife_w: Vec<f64> = ages
            .iter()
            .zip(&age_w)
            .map(|(&a, &w)| {
                if a >= MIN_COUPLE_ARRIVAL_AGE {
                    w * partnered(Sex::Female, a)
                } else {
                    0.0
                }
            })
            .collect();
        let mut pairs: Vec<(i32, i32, u64)> = Vec::new();
        for (&aw, &n) in ages.iter().zip(&apportion(couples, &wife_w)) {
            if n == 0 {
                continue;
            }
            let husband_w: Vec<f64> = ages
                .iter()
                .zip(&age_w)
                .map(|(&am, &w)| {
                    if am - aw >= ARRIVAL_GAP_MIN {
                        age_gap_weight(am - aw) * w * partnered(Sex::Male, am)
                    } else {
                        0.0
                    }
                })
                .collect();
            for (&am, &k) in ages.iter().zip(&apportion(n, &husband_w)) {
                if k > 0 {
                    pairs.push((aw, am, k));
                }
            }
        }
        // Singles carry the rest of the era's sex ratio.
        let single_men = ((men_share * (2 * couples + singles) as f64).round() as i64
            - couples as i64)
            .clamp(0, singles as i64) as u64;
        let single_f = apportion(singles - single_men, &age_w);
        let single_m = apportion(single_men, &age_w);
        // Per block: couple women, couple men, single women, single men.
        let mut per_block: BTreeMap<usize, [u64; 4]> = BTreeMap::new();
        for &(aw, am, n) in &pairs {
            per_block.entry(block_of(t - aw, r)).or_default()[0] += n;
            per_block.entry(block_of(t - am, r)).or_default()[1] += n;
        }
        for (i, &a) in ages.iter().enumerate() {
            if single_f[i] + single_m[i] > 0 {
                let e = per_block.entry(block_of(t - a, r)).or_default();
                e[2] += single_f[i];
                e[3] += single_m[i];
            }
        }
        let mut cohort_of: FxHashMap<usize, u16> = FxHashMap::default();
        for (&b, &[cf, cm, sf, sm]) in &per_block {
            let (f, m) = (cf + sf, cm + sm);
            cohort_of.insert(b, blocks[b].cohorts.len() as u16);
            blocks[b].cohorts.push(Cohort {
                arrival: Some(t),
                size: f + m,
                females: f,
                mothers: Vec::new(),
            });
            blocks[b].size += f + m;
            blocks[b].females += f;
            let mut pool = CohortPool::new(f, m, [sf as f64, sm as f64]);
            pool.used = [cf, cm];
            pools[b].cohorts.push(pool);
            let year = blocks[b].year;
            add_nonunion_births(births, b as u32, f, year, t + 1, params.y1);
        }
        // Arrival cells, and the plans' births: in-world later, or abroad
        // (children arriving now).
        let mut cells: BTreeMap<(u32, u32), u64> = BTreeMap::new();
        for &(aw, am, n) in &pairs {
            *cells
                .entry((block_of(t - aw, r) as u32, block_of(t - am, r) as u32))
                .or_default() += n;
        }
        // Each slice's couples split over dissolution classes counted from
        // arrival. Arriving adults have no in-world kin, so kin repair never
        // moves them and their classes need no de-isolation.
        let shares = SystematicShares::new(&dissolution_class_pmf(t, false));
        let mut by_f: BTreeMap<u32, Vec<(u8, u32, u64)>> = BTreeMap::new();
        let mut by_m: BTreeMap<u32, Vec<(u8, u32, u64)>> = BTreeMap::new();
        for (&(bf, bm), &n) in &cells {
            let k = key
                .with(TAG_ARRIVAL_CLASS)
                .with3(t as u64, bf as u64, bm as u64);
            shares.for_each_part(n, k, |c, nc| {
                by_f.entry(bf).or_default().push((c as u8, bm, nc));
                by_m.entry(bm).or_default().push((c as u8, bf, nc));
            });
        }
        // Children born abroad, by (child block, mother block).
        let mut kids: BTreeMap<(usize, u32), u64> = BTreeMap::new();
        let ckey = key.with(TAG_CONTINGENCY);
        for (bf, mut partners) in by_f {
            let n: u64 = partners.iter().map(|p| p.2).sum();
            let cohort = cohort_of[&(bf as usize)];
            let age = t - blocks[bf as usize].year;
            let cells = class_cells(
                t,
                CellKind::Arrival,
                &mut partners,
                &[(cohort as u32, n)],
                Members::Cohorts,
                ckey.with3(t as u64, bf as u64, 0),
            );
            for cell in cells {
                let pk = plan_key(plans.root, bf, t, CellKind::Arrival, cell.class);
                for leaf in arrival_plans(cell.total, t, age, pk, dens, plans.tables) {
                    let (in_world, abroad) = arrival_births(&leaf, age, cell.class);
                    let count = leaf.plan.count;
                    for o in bits(in_world) {
                        let year = t + o;
                        if year <= params.y1 {
                            births.at(year, bf).0 += count;
                        }
                    }
                    for c in bits(abroad) {
                        *kids.entry((block_of(t - c, r), bf)).or_default() += count;
                    }
                }
                add_source(
                    &mut blocks[bf as usize],
                    &mut pools[bf as usize],
                    Sex::Female,
                    &cell,
                );
                blocks[bf as usize].union_f.push(cell);
            }
        }
        for (bm, mut partners) in by_m {
            let n: u64 = partners.iter().map(|p| p.2).sum();
            let cohort = cohort_of[&(bm as usize)];
            let cells = class_cells(
                t,
                CellKind::Arrival,
                &mut partners,
                &[(cohort as u32, n)],
                Members::Cohorts,
                ckey.with3(t as u64, bm as u64, 1),
            );
            for cell in cells {
                add_source(
                    &mut blocks[bm as usize],
                    &mut pools[bm as usize],
                    Sex::Male,
                    &cell,
                );
                blocks[bm as usize].union_m.push(cell);
            }
        }
        // The children's cohorts: one per child block, its mothers in order.
        let mut by_child: Vec<(usize, Vec<MotherShare>)> = Vec::new();
        for ((b, mother), n) in kids {
            let share = MotherShare {
                mother,
                union_births: n,
                nonunion_births: 0,
            };
            match by_child.last_mut() {
                Some((x, list)) if *x == b => list.push(share),
                _ => by_child.push((b, vec![share])),
            }
        }
        for (b, mothers) in by_child {
            let size: u64 = mothers.iter().map(|m| m.union_births).sum();
            let male = params.male_share_at_birth;
            let f = apportion(size, &[1.0 - male, male])[0];
            blocks[b].cohorts.push(Cohort {
                arrival: Some(t),
                size,
                females: f,
                mothers,
            });
            blocks[b].size += size;
            blocks[b].females += f;
            pools[b]
                .cohorts
                .push(CohortPool::new(f, size - f, [f as f64, (size - f) as f64]));
            let year = blocks[b].year;
            add_nonunion_births(births, b as u32, f, year, t + 1, params.y1);
        }
    }
}

/// Set bits of a mask, as offsets.
fn bits(mut mask: u32) -> impl Iterator<Item = i32> {
    std::iter::from_fn(move || {
        (mask != 0).then(|| {
            let b = mask.trailing_zeros() as i32;
            mask &= mask - 1;
            b
        })
    })
}

/// Record a block's non-union births in the years `[from, to]`.
fn add_nonunion_births(
    births: &mut Births,
    mother: u32,
    females: u64,
    block_year: i32,
    from: i32,
    to: i32,
) {
    for leaf in nonunion_plans(females, block_year) {
        for k in 0..leaf.births as usize {
            let year = block_year + leaf.ages[k] as i32;
            if (from..=to).contains(&year) {
                births.at(year, mother).1 += leaf.count;
            }
        }
    }
}

/// A market seeker: a block's never-partnered or divorced members of one
/// sex (R1c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Who {
    block: u32,
    divorced: bool,
}

impl Who {
    /// Key code for rounding: the block and the status.
    fn code(self) -> u64 {
        (self.block as u64) << 1 | self.divorced as u64
    }
}

/// A market: opposite-sex (rows women, columns men), or one sex's same-sex
/// market (rows its left half, columns its right).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Market {
    Opposite,
    Same(Sex),
}

impl Market {
    /// Sexes of the rows and the columns.
    fn sexes(self) -> (Sex, Sex) {
        match self {
            Market::Opposite => (Sex::Female, Sex::Male),
            Market::Same(s) => (s, s),
        }
    }

    /// Kinds of the row's and the column's cells for a couple.
    fn kinds(self, row: Who, col: Who) -> (CellKind, CellKind) {
        match self {
            Market::Opposite => (
                CellKind::of_status(row.divorced, col.divorced),
                CellKind::of_status(col.divorced, row.divorced),
            ),
            Market::Same(_) => (CellKind::SameLeft, CellKind::SameRight),
        }
    }

    /// Kernel over the birth-year gap `gap` (column's age minus row's) and
    /// the two statuses.
    fn kernel(self, gap: i32, row: bool, col: bool) -> f64 {
        match self {
            Market::Opposite => {
                let g = if row || col {
                    remarriage_gap_weight(gap)
                } else {
                    age_gap_weight(gap)
                };
                g * status_affinity(row, col)
            }
            Market::Same(_) => same_sex_gap_weight(gap),
        }
    }
}

/// A market's couples by row and column seeker.
type Pairs = FxHashMap<(Who, Who), u64>;

/// Clear one year's markets and record the merged cells.
///
/// `want_f[b]` / `want_m[b]` are block `b`'s desired first unions this year
/// (`None`: not in the market), and `div_f` / `div_m` its divorced members'
/// desired second unions (R1c). A share `same_sex_share(t)` of first-union
/// wants goes to its sex's same-sex market; of the rest, and of all
/// second-union wants, `national_market_share(t)` goes to the national
/// opposite-sex market and the remainder to the block's regional one.
/// Regional markets clear first, then the national one, then the same-sex
/// ones, each against what the earlier ones left. The divorced and the
/// never-partnered meet in the same opposite-sex markets; a couple's cell
/// kinds follow from their statuses. Each market's integer matrix is summed
/// per (row block, column block, kinds), split into dissolution classes and
/// recorded as one cell per block, sex, kind and class.
#[allow(clippy::too_many_arguments)]
fn clear_year(
    t: i32,
    same_sex_boost: f64,
    (want_f, want_m): (&[Option<f64>], &[Option<f64>]),
    (div_f, div_m): (&[Option<f64>], &[Option<f64>]),
    regions: usize,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut Births,
    y1: i32,
    key: Key,
    plans: Plans,
    class_moves: &mut u64,
) {
    let sigma = (same_sex_share(t) * same_sex_boost).min(0.5);
    let rho = if regions > 1 {
        national_market_share(t)
    } else {
        0.0
    };
    let parts = |never: &[Option<f64>],
                 div: &[Option<f64>],
                 market: Option<u16>,
                 (share_n, share_d): (f64, f64)|
     -> Vec<(Who, f64)> {
        let mut out = Vec::new();
        for (b, (&n, &d)) in never.iter().zip(div).enumerate() {
            if !market.map_or(true, |r| blocks[b].region == r) {
                continue;
            }
            for (w, share, divorced) in [(n, share_n, false), (d, share_d, true)] {
                if let Some(w) = w.map(|w| w * share).filter(|&w| w > 0.0) {
                    let who = Who {
                        block: b as u32,
                        divorced,
                    };
                    out.push((who, w));
                }
            }
        }
        out
    };
    let none = vec![None; want_f.len()];
    // Every market's participants, before any cell is recorded.
    let local = ((1.0 - sigma) * (1.0 - rho), 1.0 - rho);
    let national = ((1.0 - sigma) * rho, rho);
    let regional: Vec<_> = (0..regions as u16)
        .map(|r| {
            (
                parts(want_f, div_f, Some(r), local),
                parts(want_m, div_m, Some(r), local),
            )
        })
        .collect();
    let nationwide = (
        parts(want_f, div_f, None, national),
        parts(want_m, div_m, None, national),
    );
    let halves = [
        (Sex::Female, parts(want_f, &none, None, (sigma / 2.0, 0.0))),
        (Sex::Male, parts(want_m, &none, None, (sigma / 2.0, 0.0))),
    ];

    // Every market's matrix depends only on its participants (fixed above)
    // and the blocks' birth years, so all of them are solved at once; the
    // caps then apply in order, since they depend on earlier takings.
    let mut specs: Vec<MarketSpec> = regional
        .iter()
        .enumerate()
        .map(|(r, (f, m))| MarketSpec {
            mk: r as u64,
            rows: f,
            cols: m,
            market: Market::Opposite,
        })
        .collect();
    if rho > 0.0 {
        specs.push(MarketSpec {
            mk: regions as u64,
            rows: &nationwide.0,
            cols: &nationwide.1,
            market: Market::Opposite,
        });
    }
    let opposite_markets = specs.len();
    for (sex, half) in &halves {
        specs.push(MarketSpec {
            mk: regions as u64 + 1 + *sex as u64,
            rows: half,
            cols: half,
            market: Market::Same(*sex),
        });
    }
    let solved: Vec<Vec<u64>> = {
        let blocks: &[Block] = blocks;
        specs
            .par_iter()
            .map(|spec| solve_market(t, spec, blocks, key))
            .collect()
    };
    let mut solved = specs.iter().zip(solved);

    let mut opposite = Pairs::default();
    for (spec, x) in solved.by_ref().take(opposite_markets) {
        apply_market(t, spec, x, pools, &mut opposite);
    }
    defer_isolated(&mut opposite, Market::Opposite, pools);
    record(
        t,
        &opposite,
        Market::Opposite,
        blocks,
        pools,
        births,
        y1,
        (key, plans),
        class_moves,
    );

    // Same-sex markets: each sex's seekers split into a left and a right
    // half, paired like women and men.
    for (spec, x) in solved {
        let mut pairs = Pairs::default();
        apply_market(t, spec, x, pools, &mut pairs);
        defer_isolated(&mut pairs, spec.market, pools);
        record(
            t,
            &pairs,
            spec.market,
            blocks,
            pools,
            births,
            y1,
            (key, plans),
            class_moves,
        );
    }
}

/// A market group's couples by slice and dissolution class: `(row block,
/// column block, class, couples)`, nonzero only, sorted.
type ClassSplit = Vec<(u32, u32, u8, u64)>;

/// Split each slice's couples over dissolution classes by keyed systematic
/// apportionment (R1c): exact per slice, unbiased, and within one of the
/// expected count, so even one-couple slices divorce at the right rate.
/// `shares` are the class pmf's ([`SystematicShares`]); most slices hold one
/// couple, so only their nonzero classes are found.
fn split_classes(
    pairs: &FxHashMap<(u32, u32), u64>,
    shares: &SystematicShares,
    key: Key,
) -> ClassSplit {
    let mut slices: Vec<((u32, u32), u64)> = pairs.iter().map(|(&k, &n)| (k, n)).collect();
    slices.sort_unstable_by_key(|x| x.0);
    // Each slice's split is keyed by the slice alone: chunks run in parallel.
    let parts: Vec<ClassSplit> = slices
        .par_chunks(4096)
        .map(|chunk| {
            let mut out = Vec::with_capacity(chunk.len() + chunk.len() / 4);
            for &((a, b), n) in chunk {
                shares.for_each_part(n, key.with2(a as u64, b as u64), |c, k| {
                    out.push((a, b, c as u8, k));
                });
            }
            out
        })
        .collect();
    parts.concat()
}

/// A side's partners by block: `(block, [(class, partner block, couples)])`.
type ByBlock = Vec<(u32, Vec<(u8, u32, u64)>)>;

/// A group's couples on one side, by block in ascending order: `side(e)`
/// gives an entry's block and its `(class, partner block, couples)`. A
/// counting sort, since blocks are small integers; each list keeps the
/// split's order (class cells sort their own).
fn by_block(
    split: &ClassSplit,
    side: impl Fn(&(u32, u32, u8, u64)) -> (u32, (u8, u32, u64)),
) -> ByBlock {
    let Some(max) = split.iter().map(|e| side(e).0).max() else {
        return Vec::new();
    };
    let mut count = vec![0u32; max as usize + 1];
    for e in split {
        count[side(e).0 as usize] += 1;
    }
    let mut out = Vec::new();
    let mut slot = vec![u32::MAX; max as usize + 1];
    for (b, &k) in count.iter().enumerate() {
        if k > 0 {
            slot[b] = out.len() as u32;
            out.push((b as u32, Vec::with_capacity(k as usize)));
        }
    }
    for e in split {
        let (b, p) = side(e);
        out[slot[b as usize] as usize].1.push(p);
    }
    out
}

/// A market's couples in class 0 only (same-sex markets, whose
/// dissolution is a keyed draw).
fn one_class(pairs: &FxHashMap<(u32, u32), u64>) -> ClassSplit {
    let mut out: ClassSplit = pairs.iter().map(|(&(a, b), &n)| (a, b, 0, n)).collect();
    out.sort_unstable();
    out
}

/// Move every couple that is alone on both sides of its dissolution class
/// (the only one of its row block's cell in that class, and the only one of
/// its column block's) to the nearest class in which either cell has
/// another couple: kin repair works within classes, and such a couple has no
/// count-preserving repair. Year classes move to the nearest year (earlier
/// on ties), else class 0; class 0 moves to the year class with the most
/// couples. Returns the couples moved.
///
/// One pass suffices: a move empties two singleton sub-cells and only adds
/// to occupied ones, so no couple becomes isolated. Couples alone in both
/// *cells* were deferred before ([`defer_isolated`]), so a target always
/// exists.
fn deisolate(split: &mut ClassSplit) -> u64 {
    const C: usize = MAX_CLASS + 1;
    // Dense row and column indices; couples per (row block, class) and
    // (column block, class).
    let mut row_blocks: Vec<u32> = split.iter().map(|e| e.0).collect();
    row_blocks.dedup();
    let mut col_blocks: Vec<u32> = split.iter().map(|e| e.1).collect();
    col_blocks.sort_unstable();
    col_blocks.dedup();
    let mut col_index = vec![u32::MAX; col_blocks.last().map_or(0, |&b| b as usize + 1)];
    for (i, &b) in col_blocks.iter().enumerate() {
        col_index[b as usize] = i as u32;
    }
    let col_of = |b: u32| col_index[b as usize] as usize;
    let mut rows = vec![0u64; row_blocks.len() * C];
    let mut cols = vec![0u64; col_blocks.len() * C];
    let mut ri = 0;
    for &(a, b, c, n) in split.iter() {
        while row_blocks[ri] != a {
            ri += 1;
        }
        rows[ri * C + c as usize] += n;
        cols[col_of(b) * C + c as usize] += n;
    }
    // Slice by slice, classes in ascending order (a move to a later class is
    // visited again), rewritten into a new list.
    let mut out: ClassSplit = Vec::with_capacity(split.len());
    let mut moved = 0;
    let (mut i, mut ri) = (0, 0);
    while i < split.len() {
        let (a, b) = (split[i].0, split[i].1);
        while row_blocks[ri] != a {
            ri += 1;
        }
        let (r0, c0) = (ri * C, col_of(b) * C);
        let mut v = [0u64; C];
        let mut mask = 0u64;
        while i < split.len() && (split[i].0, split[i].1) == (a, b) {
            v[split[i].2 as usize] = split[i].3;
            mask |= 1 << split[i].2;
            i += 1;
        }
        let mut todo = mask;
        while todo != 0 {
            let c = todo.trailing_zeros() as usize;
            todo &= todo - 1;
            let isolated = v[c] == 1 && rows[r0 + c] == 1 && cols[c0 + c] == 1;
            if !isolated {
                continue;
            }
            let occupied = |k: usize| k != c && (rows[r0 + k] > 0 || cols[c0 + k] > 0);
            let target = if c == 0 {
                (1..=MAX_CLASS)
                    .filter(|&k| occupied(k))
                    .max_by_key(|&k| (rows[r0 + k] + cols[c0 + k], std::cmp::Reverse(k)))
            } else {
                (1..=MAX_CLASS)
                    .filter(|&k| occupied(k))
                    .min_by_key(|&k| (k.abs_diff(c), k))
                    .or_else(|| occupied(0).then_some(0))
            };
            let Some(k) = target else { continue };
            v[c] -= 1;
            v[k] += 1;
            rows[r0 + c] -= 1;
            cols[c0 + c] -= 1;
            rows[r0 + k] += 1;
            cols[c0 + k] += 1;
            moved += 1;
            mask |= 1 << k;
            if k > c {
                todo |= 1 << k;
            }
        }
        while mask != 0 {
            let c = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            if v[c] > 0 {
                out.push((a, b, c as u8, v[c]));
            }
        }
    }
    *split = out;
    moved
}

/// Defer isolated couples: a row member alone in their cell with a column
/// member alone in theirs (cells being per block and kind). No
/// count-preserving swap can repair such a couple if they are kin, so the
/// union is not formed this year; both stay in their pools for the next
/// market (R1 plan, kin repair). Removing it empties both cells, so no new
/// isolated couple appears.
fn defer_isolated(pairs: &mut Pairs, market: Market, pools: &mut [Pool]) {
    let (rsex, csex) = market.sexes();
    let mut per_row: FxHashMap<(u32, CellKind), u64> = FxHashMap::default();
    let mut per_col: FxHashMap<(u32, CellKind), u64> = FxHashMap::default();
    for (&(rw, cw), &n) in pairs.iter() {
        let (kf, km) = market.kinds(rw, cw);
        *per_row.entry((rw.block, kf)).or_default() += n;
        *per_col.entry((cw.block, km)).or_default() += n;
    }
    pairs.retain(|&(rw, cw), &mut n| {
        let (kf, km) = market.kinds(rw, cw);
        let isolated = n == 1 && per_row[&(rw.block, kf)] == 1 && per_col[&(cw.block, km)] == 1;
        if isolated {
            pools[rw.block as usize].taken[kf as usize][rsex as usize] -= 1;
            pools[cw.block as usize].taken[km as usize][csex as usize] -= 1;
        }
        !isolated
    });
}

/// Record one market's couples: grouped by the two cells' kinds, each
/// group's slices split over dissolution classes (opposite-sex) and
/// de-isolated, each block's takings split over its entry cohorts
/// (never-partnered kinds) or divorced sources (second unions), and the
/// class-cells recorded on both sides. Also the women's plan births per
/// class-cell, the remarriage parts of the sources feeding second unions,
/// and the new divorced sources (R1c).
#[allow(clippy::too_many_arguments)]
fn record(
    t: i32,
    pairs: &Pairs,
    market: Market,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut Births,
    y1: i32,
    (key, plans): (Key, Plans),
    class_moves: &mut u64,
) {
    let (rsex, csex) = market.sexes();
    let mut groups: BTreeMap<(CellKind, CellKind), FxHashMap<(u32, u32), u64>> = BTreeMap::new();
    for (&(rw, cw), &n) in pairs {
        *groups
            .entry(market.kinds(rw, cw))
            .or_default()
            .entry((rw.block, cw.block))
            .or_default() += n;
    }
    for ((kf, km), slices) in groups {
        let mut split = match market {
            Market::Opposite => split_classes(
                &slices,
                &SystematicShares::new(&dissolution_class_pmf(t, kf.remarriage())),
                key.with3(TAG_CLASS, t as u64, kf as u64),
            ),
            Market::Same(_) => one_class(&slices),
        };
        if market == Market::Opposite {
            *class_moves += deisolate(&mut split);
        }
        // Each side's partners by block, ascending: `(class, partner block,
        // couples)`.
        let by_row = by_block(&split, |&(a, b, c, n)| (a, (c, b, n)));
        let by_col = by_block(&split, |&(a, b, c, n)| (b, (c, a, n)));
        for (sex, kind, grouped) in [(rsex, kf, by_row), (csex, km, by_col)] {
            // Each block's cells, plans and sources touch only that block
            // and its pool, so the blocks are recorded on every core; their
            // births (sums) are merged after, so the result is the same.
            // Tasks borrow their partner lists, so the lists are freed here,
            // not across threads.
            let mut grouped = grouped;
            let mut tasks = Vec::with_capacity(grouped.len());
            let mut next = grouped.iter_mut().peekable();
            for (b, (block, pool)) in blocks.iter_mut().zip(pools.iter_mut()).enumerate() {
                if next.peek().is_some_and(|g| g.0 as usize == b) {
                    let (_, partners) = next.next().unwrap();
                    tasks.push((b as u32, partners, block, pool));
                }
            }
            assert!(next.next().is_none(), "every block recorded");
            let side = Side {
                t,
                sex,
                kind,
                key,
                plans,
            };
            let born: Vec<(u32, [u64; 32])> = tasks
                .into_par_iter()
                .map(|(b, partners, block, pool)| (b, side.record_block(b, partners, block, pool)))
                .collect();
            for (b, by_offset) in born {
                for (o, &n) in by_offset.iter().enumerate() {
                    let year = t + o as i32;
                    if n > 0 && year <= y1 {
                        births.at(year, b).0 += n;
                    }
                }
            }
        }
    }
}

/// One side of a market group being recorded: the year, the side's sex and
/// cell kind, and the keys.
#[derive(Clone, Copy)]
struct Side<'a> {
    t: i32,
    sex: Sex,
    kind: CellKind,
    key: Key,
    plans: Plans<'a>,
}

impl Side<'_> {
    /// Record block `b`'s side of a group, from its partners `(class,
    /// partner block, couples)`: split its takings over its cohorts or
    /// divorced sources, lay out its class-cells, register the sources they
    /// draw on and the new ones they make, and return the women's plan
    /// births by offset from the union year.
    fn record_block(
        &self,
        b: u32,
        partners: &mut [(u8, u32, u64)],
        block: &mut Block,
        pool: &mut Pool,
    ) -> [u64; 32] {
        let Self {
            t,
            sex,
            kind,
            key,
            plans,
        } = *self;
        let n: u64 = partners.iter().map(|p| p.2).sum();
        let age = t - block.year;
        let (members, from) = if kind.second() {
            let skey =
                key.with(TAG_SETTLE)
                    .with3(t as u64, b as u64, (kind as u64) << 1 | sex as u64);
            (pool.settle_div(kind, sex, t, skey), Members::Sources)
        } else {
            (pool.settle(kind, sex), Members::Cohorts)
        };
        debug_assert_eq!(members.iter().map(|c| c.1).sum::<u64>(), n);
        let cells = class_cells(
            t,
            kind,
            partners,
            &members,
            from,
            key.with(TAG_CONTINGENCY)
                .with3(t as u64, b as u64, (kind as u64) << 1 | sex as u64),
        );
        let mut born = [0u64; 32];
        let mut leaves: Vec<PlanLeaf> = Vec::new();
        for cell in cells {
            // Opposite-sex women's plans give births; same-sex unions have
            // none in R1.
            if sex == Sex::Female && !kind.same_sex() {
                let pk = plan_key(plans.root, b, t, kind, cell.class);
                leaves.clear();
                plans
                    .tables
                    .year(t)
                    .plans_into(cell.total, kind.second(), pk, &mut leaves);
                for leaf in &leaves {
                    let (offs, nb) = leaf_births(leaf, age, cutoff(cell.class));
                    for &o in &offs[..nb] {
                        born[o as usize] += leaf.count;
                    }
                }
            }
            // The sources feeding a second-union cell record where their
            // members went.
            let div = match sex {
                Sex::Female => &mut block.div_f,
                Sex::Male => &mut block.div_m,
            };
            for &(src, count) in &cell.sources {
                div[src as usize].parts.push((t, kind, cell.class, count));
            }
            add_source(block, pool, sex, &cell);
            match sex {
                Sex::Female => block.union_f.push(cell),
                Sex::Male => block.union_m.push(cell),
            }
        }
        born
    }
}

/// One of a year's markets: its key, its seekers (rows and columns, each
/// with a desired number of unions) and its kind.
struct MarketSpec<'a> {
    mk: u64,
    rows: &'a [(Who, f64)],
    cols: &'a [(Who, f64)],
    market: Market,
}

/// Solve one market: two-sided totals, IPF over the kernel (by birth-year
/// gap and statuses) and keyed rounding. Returns the row-major integer
/// matrix before caps (empty if the market is). A pure function of the
/// seekers and the blocks' birth years.
fn solve_market(t: i32, spec: &MarketSpec, blocks: &[Block], key: Key) -> Vec<u64> {
    let MarketSpec {
        mk,
        rows,
        cols,
        market,
    } = *spec;
    let (nr, nc) = (rows.len(), cols.len());
    if nr == 0 || nc == 0 {
        return Vec::new();
    }
    let (sf, sm): (f64, f64) = (
        rows.iter().map(|p| p.1).sum(),
        cols.iter().map(|p| p.1).sum(),
    );
    if sf <= 0.0 || sm <= 0.0 {
        return Vec::new();
    }
    let total = 2.0 * sf * sm / (sf + sm);
    let r: Vec<f64> = rows.iter().map(|p| p.1 * total / sf).collect();
    let c: Vec<f64> = cols.iter().map(|p| p.1 * total / sm).collect();
    // The kernel depends only on the birth-year gap and the statuses, so
    // the IPF runs over (birth year, status) groups and each seeker takes
    // its share of its group's margin: the fixed point `a_i b_j g(...)` has
    // `a_i ∝ r_i` within a group, so `x_ij = X[g_i][g_j] · r_i / R[g_i] ·
    // c_j / C[g_j]` exactly (R1 plan, R1b-1).
    let group = |parts: &[(Who, f64)], margin: &[f64]| {
        let mut keys: Vec<(i32, bool)> = parts
            .iter()
            .map(|&(w, _)| (blocks[w.block as usize].year, w.divorced))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        let idx: Vec<usize> = parts
            .iter()
            .map(|&(w, _)| {
                keys.binary_search(&(blocks[w.block as usize].year, w.divorced))
                    .unwrap()
            })
            .collect();
        let mut sums = vec![0.0; keys.len()];
        for (&g, &v) in idx.iter().zip(margin) {
            sums[g] += v;
        }
        (keys, idx, sums)
    };
    let (keys_f, gi, rs) = group(rows, &r);
    let (keys_m, gj, cs) = group(cols, &c);
    let (gr, gc) = (keys_f.len(), keys_m.len());
    let mut g = vec![0.0; gr * gc];
    for (a, &(yf, df)) in keys_f.iter().enumerate() {
        for (b, &(ym, dm)) in keys_m.iter().enumerate() {
            // age_col - age_row = year_row - year_col
            g[a * gc + b] = market.kernel(yf - ym, df, dm);
        }
    }
    ipf(&mut g, gr, gc, &rs, &cs);
    let share = |v: f64, sum: f64| if sum > 0.0 { v / sum } else { 0.0 };

    // Keyed rounding: floor plus a Bernoulli on the fraction.
    let key = key.with2(t as u64, mk);
    // A zero cell has no fraction, so it needs no draw (`unit() < 0` is
    // never true).
    // Each row is a pure function of its seeker, so rows round in parallel.
    let mut x = vec![0u64; nr * nc];
    let col_share: Vec<f64> = (0..nc).map(|j| share(c[j], cs[gj[j]])).collect();
    x.par_chunks_mut(nc).enumerate().for_each(|(i, xrow)| {
        let rw = rows[i].0;
        let (grow, sr) = (&g[gi[i] * gc..(gi[i] + 1) * gc], share(r[i], rs[gi[i]]));
        for (j, (&(cw, _), out)) in cols.iter().zip(xrow.iter_mut()).enumerate() {
            let v = grow[gj[j]] * sr * col_share[j];
            let fl = v.floor();
            *out = if v > fl {
                let up = key.with2(rw.code(), cw.code()).unit() < v - fl;
                fl as u64 + up as u64
            } else {
                fl as u64
            };
        }
    });
    x
}

/// Apply a solved market's matrix `x`: capacity caps, then the takings in
/// the pools by cell kind, and the couples added to `pairs`. Rows' takings
/// count against the columns' caps when both sides share a pool (same-sex
/// markets), so a pool is never over-drawn.
fn apply_market(t: i32, spec: &MarketSpec, mut x: Vec<u64>, pools: &mut [Pool], pairs: &mut Pairs) {
    if x.is_empty() {
        return;
    }
    let MarketSpec {
        rows, cols, market, ..
    } = *spec;
    let (nr, nc) = (rows.len(), cols.len());
    let (rsex, csex) = market.sexes();
    // Caps: never more unions than expected available survivors, and never
    // more than the seeker's available members. Rows first; their takings
    // count against the columns' caps when both sides share a pool.
    let mut row_take: FxHashMap<Who, u64> = FxHashMap::default();
    for (i, &(rw, _)) in rows.iter().enumerate() {
        let row = &mut x[i * nc..(i + 1) * nc];
        trim(row, pools[rw.block as usize].cap(rsex, rw.divorced, t));
        *row_take.entry(rw).or_default() += row.iter().sum::<u64>();
    }
    for (j, &(cw, _)) in cols.iter().enumerate() {
        let shared = match market {
            Market::Same(_) => row_take.get(&cw).copied().unwrap_or(0),
            Market::Opposite => 0,
        };
        let cap = pools[cw.block as usize]
            .cap(csex, cw.divorced, t)
            .saturating_sub(shared);
        let mut col: Vec<u64> = (0..nr).map(|i| x[i * nc + j]).collect();
        trim(&mut col, cap);
        for (i, v) in col.into_iter().enumerate() {
            let cut = x[i * nc + j] - v;
            if cut > 0 {
                x[i * nc + j] = v;
                *row_take.get_mut(&rows[i].0).unwrap() -= cut;
            }
        }
    }

    // Record the takings by cell kind and add the matrix to the pairs.
    for (i, &(rw, _)) in rows.iter().enumerate() {
        for (j, &(cw, _)) in cols.iter().enumerate() {
            let n = x[i * nc + j];
            if n > 0 {
                let (kf, km) = market.kinds(rw, cw);
                pools[rw.block as usize].taken[kf as usize][rsex as usize] += n;
                pools[cw.block as usize].taken[km as usize][csex as usize] += n;
                *pairs.entry((rw, cw)).or_default() += n;
            }
        }
    }
}

/// Reduce `cells` until they sum to at most `cap`, taking from the largest
/// cell first (ties: lowest index).
fn trim(cells: &mut [u64], cap: u64) {
    let mut sum: u64 = cells.iter().sum();
    if sum <= cap {
        return;
    }
    // Cutting the largest cell either empties it or ends the trim, so the
    // cells are taken once each in (size descending, index) order.
    let mut order: Vec<usize> = (0..cells.len()).filter(|&i| cells[i] > 0).collect();
    order.sort_unstable_by(|&a, &b| cells[b].cmp(&cells[a]).then(a.cmp(&b)));
    for i in order {
        if sum <= cap {
            break;
        }
        let cut = (sum - cap).min(cells[i]);
        cells[i] -= cut;
        sum -= cut;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_ledger_is_consistent() {
        let l = Ledger::build(Params::tiny(), 1);
        // Natives are exactly the births; cohorts add up to the block.
        for b in &l.blocks {
            if !b.founder {
                let s: u64 = b
                    .mothers
                    .iter()
                    .map(|m| m.union_births + m.nonunion_births)
                    .sum();
                assert_eq!(s, b.cohorts[0].size, "block {}", b.year);
            }
            assert_eq!(b.cohorts.iter().map(|c| c.size).sum::<u64>(), b.size);
            assert_eq!(b.cohorts.iter().map(|c| c.females).sum::<u64>(), b.females);
            assert!(b.cohorts[1..].iter().all(|c| c.arrival.is_some()));
        }
        // Cells split exactly over cohorts; nobody partners before the
        // year after arrival, and no cohort partners more members than it has.
        let immigrants: u64 = l
            .blocks
            .iter()
            .flat_map(|b| &b.cohorts[1..])
            .map(|c| c.size)
            .sum();
        assert!(immigrants > 500, "only {immigrants} immigrants");
        for b in &l.blocks {
            let mut used = vec![[0u64; 2]; b.cohorts.len()];
            for (s, (cells, divs)) in [(&b.union_f, &b.div_f), (&b.union_m, &b.div_m)]
                .into_iter()
                .enumerate()
            {
                for c in cells {
                    // Second unions (R1c) come from divorced sources that
                    // separated before the cell's year; first unions from
                    // entry cohorts.
                    if c.kind.second() {
                        assert!(c.cohorts.is_empty());
                        assert_eq!(c.sources.iter().map(|x| x.1).sum::<u64>(), c.total);
                        for &(src, _) in &c.sources {
                            assert!(divs[src as usize].divorce_year() < c.year);
                        }
                        continue;
                    }
                    assert!(c.sources.is_empty());
                    assert_eq!(c.cohorts.iter().map(|x| x.1).sum::<u64>(), c.total);
                    for &(co, n) in &c.cohorts {
                        if let Some(t) = b.cohorts[co as usize].arrival {
                            // Arrival cells hold the couples arriving that
                            // year; otherwise unions follow arrival.
                            let ok = if c.kind == CellKind::Arrival {
                                c.year == t
                            } else {
                                c.year > t
                            };
                            assert!(ok, "union in {} vs arrival {t}", c.year);
                        }
                        used[co as usize][s] += n;
                    }
                }
            }
            for (co, u) in b.cohorts.iter().zip(&used) {
                assert!(u[0] <= co.females && u[1] <= co.size - co.females);
            }
            // Pool accounting (R1c): each source's parts are its members
            // who re-partner, recorded in second-union cells of later years,
            // and no more than it has; the cells' source counts match.
            for (cells, divs) in [(&b.union_f, &b.div_f), (&b.union_m, &b.div_m)] {
                let mut fed = vec![0u64; divs.len()];
                for c in cells.iter().filter(|c| c.kind.second()) {
                    for &(src, n) in &c.sources {
                        fed[src as usize] += n;
                    }
                }
                for (d, &f) in divs.iter().zip(&fed) {
                    let parts: u64 = d.parts.iter().map(|p| p.3).sum();
                    assert_eq!(parts, f, "a source's parts match its cells");
                    assert!(parts <= d.members);
                    assert!(d.class > 0 && d.kind.first_opposite());
                    for &(y2, k2, c2, _) in &d.parts {
                        assert!(y2 > d.divorce_year() && k2.second());
                        assert!(cells
                            .iter()
                            .any(|c| (c.year, c.kind, c.class) == (y2, k2, c2)));
                    }
                }
            }
        }
        // Union cells match across sides: each left-role cell's slices
        // equal the mirror slices of the right-role cells, per (year, kind).
        type Side = FxHashMap<(i32, CellKind, u8, u32, u32), u64>;
        let (mut left, mut right): (Side, Side) = Default::default();
        for (bi, b) in l.blocks.iter().enumerate() {
            let bi = bi as u32;
            for (sex, cells) in [(Sex::Female, &b.union_f), (Sex::Male, &b.union_m)] {
                for c in cells {
                    assert_eq!(c.total, c.partners.iter().map(|p| p.1).sum::<u64>());
                    assert!(c.partners.windows(2).all(|w| w[0].0 < w[1].0));
                    if c.kind.same_sex() {
                        assert_eq!(c.class, 0, "same-sex classes");
                    }
                    let is_left = match c.kind {
                        CellKind::SameLeft => true,
                        CellKind::SameRight => false,
                        _ => sex == Sex::Female,
                    };
                    let kind = if is_left { c.kind } else { c.kind.partner() };
                    for &(pb, n) in &c.partners {
                        if is_left {
                            left.insert((c.year, kind, c.class, bi, pb), n);
                        } else {
                            right.insert((c.year, kind, c.class, pb, bi), n);
                        }
                    }
                }
            }
            // First unions never exceed the members (second unions reuse
            // divorced members, R1c).
            let first = |cells: &Vec<UnionCell>| -> u64 {
                cells
                    .iter()
                    .filter(|c| !c.kind.second())
                    .map(|c| c.total)
                    .sum()
            };
            assert!(first(&b.union_f) <= b.females);
            assert!(first(&b.union_m) <= b.size - b.females);
            // Cells are in (year, kind, class) order, one per kind, class
            // and year.
            for cells in [&b.union_f, &b.union_m] {
                assert!(cells.windows(2).all(
                    |w| (w[0].year, w[0].kind, w[0].class) < (w[1].year, w[1].kind, w[1].class)
                ));
            }
        }
        assert_eq!(left, right);
        assert!(
            left.keys().any(|k| k.1 == CellKind::SameLeft),
            "no same-sex unions"
        );
        for kind in [
            CellKind::FirstWithSecond,
            CellKind::SecondWithFirst,
            CellKind::SecondWithSecond,
        ] {
            assert!(left.keys().any(|k| k.1 == kind), "no {kind:?} unions");
        }
        assert!(l.population() > 1000);
    }

    #[test]
    fn capped_apportionment_is_exact_and_capped() {
        assert_eq!(apportion_capped(10, &[1.0, 1.0], &[2, 20]), vec![2, 8]);
        assert_eq!(apportion_capped(5, &[0.0, 0.0], &[3, 3]), vec![3, 2]);
        assert_eq!(apportion_capped(0, &[1.0], &[0]), vec![0]);
        let caps = [5u64, 0, 7, 100, 1];
        for n in 0..=113 {
            let v = apportion_capped(n, &[3.0, 9.0, 0.5, 0.1, 2.0], &caps);
            assert_eq!(v.iter().sum::<u64>(), n);
            assert!(v.iter().zip(&caps).all(|(a, c)| a <= c));
        }
    }

    #[test]
    fn sweep_is_exact_capped_and_proportional() {
        let weights = [0.5, 0.0, 2.0, 1.0, 0.25];
        let caps = [3u64, 4, 2, 5, 9];
        for n in 0..=caps.iter().sum::<u64>() {
            for seed in 0..40 {
                let mut got = [0u64; 5];
                sweep(
                    n,
                    5,
                    |k| (weights[k], caps[k]),
                    Key::from_seed(seed).unit(),
                    |k, c| got[k] += c,
                );
                assert_eq!(got.iter().sum::<u64>(), n);
                assert!(got.iter().zip(&caps).all(|(g, c)| g <= c));
            }
        }
        // Small takings follow the weights on average.
        let mut sums = [0u64; 5];
        let trials = 20_000;
        for seed in 0..trials {
            sweep(
                1,
                5,
                |k| (weights[k], caps[k]),
                Key::from_seed(seed).unit(),
                |k, c| sums[k] += c,
            );
        }
        let total: f64 = weights.iter().sum();
        for (s, w) in sums.iter().zip(weights) {
            assert!((*s as f64 / trials as f64 - w / total).abs() < 0.01);
        }
    }

    #[test]
    fn trim_respects_cap() {
        let mut v = vec![5, 9, 1, 9];
        trim(&mut v, 10);
        assert_eq!(v.iter().sum::<u64>(), 10);
        let mut v = vec![1, 2];
        trim(&mut v, 10);
        assert_eq!(v, vec![1, 2]);
    }
}
