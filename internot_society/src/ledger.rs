//! The demographic ledger: integer counts both sides of every relation read.
//!
//! Built once per parameter set by a year-by-year cohort projection. It holds
//! only the counts that link blocks:
//! - **block sizes**, which *are* the births to earlier blocks;
//! - **union cells**, the two-sex market's integer matrix per year: women of
//!   block `i` who start a union with men of block `j`, with the same number
//!   recorded on both sides;
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
use rustc_hash::FxHashMap;

use std::collections::BTreeMap;

use crate::params::{
    age_gap_weight, arrival_age_weight, couple_arrival_share, death_prob, first_union_hazard,
    immigrant_male_share, immigration_rate, national_market_share, same_sex_gap_weight,
    same_sex_share, Params, Sex, ARRIVAL_GAP_MIN, MAX_AGE, MAX_ARRIVAL_AGE, MIN_ARRIVAL_AGE,
    MIN_COUPLE_ARRIVAL_AGE,
};
use crate::plan::{
    apportion, arrival_births, arrival_plans, leaf_births, nonunion_plans, union_plans,
};

/// Youngest age at which a union starts (so everyone is at least 15 on the
/// union date).
pub const MIN_UNION_AGE: i32 = 16;
/// Oldest age at which a woman starts a first union.
pub const MAX_UNION_AGE_F: i32 = 65;
/// Oldest age at which a man starts a first union.
pub const MAX_UNION_AGE_M: i32 = 70;

/// A block's union cell for one year: members who start their (first) union
/// that year, split by the partner's block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnionCell {
    /// Union year.
    pub year: i32,
    /// Members in the cell.
    pub total: u64,
    /// `(partner block, count)`, in ascending partner-block order.
    pub partners: Vec<(u32, u64)>,
    /// `(cohort, count)`: the members by entry cohort, in cohort order,
    /// nonzero only. Independent of the partner split.
    pub cohorts: Vec<(u16, u64)>,
    /// Which union cell of the year this is; cells are ordered by `(year,
    /// kind)`.
    pub kind: CellKind,
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
}

impl CellKind {
    /// The kind of the partner's cell.
    pub fn partner(self) -> CellKind {
        match self {
            CellKind::SameLeft => CellKind::SameRight,
            CellKind::SameRight => CellKind::SameLeft,
            k => k,
        }
    }

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
}

const TAG_ROUND: u64 = label("ledger/round");

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
    for _ in 0..60 {
        for i in 0..rows {
            let s: f64 = k[i * cols..(i + 1) * cols].iter().sum();
            let f = if s > 0.0 { r[i] / s } else { 0.0 };
            k[i * cols..(i + 1) * cols].iter_mut().for_each(|x| *x *= f);
        }
        for j in 0..cols {
            let s: f64 = (0..rows).map(|i| k[i * cols + j]).sum();
            let f = if s > 0.0 { c[j] / s } else { 0.0 };
            (0..rows).for_each(|i| k[i * cols + j] *= f);
        }
        let converged = (0..rows).all(|i| {
            let s: f64 = k[i * cols..(i + 1) * cols].iter().sum();
            (s - r[i]).abs() <= 1e-10 * r[i].max(1.0)
        });
        if converged {
            break;
        }
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

/// Mutable projection state for one block: its cohorts, plus the unions
/// taken this year, by market (`InWorld`, `SameLeft`, `SameRight`) and
/// sex, not yet split over the cohorts.
struct Pool {
    cohorts: Vec<CohortPool>,
    taken: [[u64; 2]; 3],
}

impl Pool {
    fn one(c: CohortPool) -> Self {
        Self {
            cohorts: vec![c],
            taken: [[0; 2]; 3],
        }
    }

    fn taken_total(&self, s: Sex) -> u64 {
        self.taken.iter().map(|t| t[s as usize]).sum()
    }

    /// Expected never-partnered members left this year.
    fn never(&self, s: Sex) -> f64 {
        let n: f64 = self
            .cohorts
            .iter()
            .map(|c| c.never[s as usize].max(0.0))
            .sum();
        n - self.taken_total(s) as f64
    }

    /// Members not yet in a union, net of this year's takings.
    fn unused(&self, s: Sex) -> u64 {
        let n: u64 = self
            .cohorts
            .iter()
            .map(|c| c.size[s as usize] - c.used[s as usize])
            .sum();
        n - self.taken_total(s)
    }

    /// Split this year's takings in one market over the cohorts: by
    /// never-partnered pool, capped by each cohort's unused members.
    /// Returns `(cohort, count)`.
    fn settle(&mut self, market: CellKind, s: Sex) -> Vec<(u16, u64)> {
        let si = s as usize;
        let n = std::mem::take(&mut self.taken[market as usize][si]);
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
                out.push((ci as u16, k));
            }
        }
        out
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
        let regions = params.region_count();
        assert!(regions >= 1, "a world needs at least one region");
        let first_year = params.y0 - params.founder_max_age;
        let years = (params.y1 - first_year + 1) as usize;
        let male_share = params.male_share_at_birth;
        let mut blocks: Vec<Block> = Vec::with_capacity(years * regions);
        let mut pools: Vec<Pool> = Vec::with_capacity(years * regions);
        // births[year - first_year]: mother block → (union, non-union)
        let mut births: Vec<FxHashMap<u32, (u64, u64)>> = vec![FxHashMap::default(); years];
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
                });
                pools.push(Pool::one(CohortPool::new(
                    f,
                    m,
                    [
                        f as f64 * never_partnered_share(Sex::Female, age, params.y0),
                        m as f64 * never_partnered_share(Sex::Male, age, params.y0),
                    ],
                )));
                add_nonunion_births(&mut births, idx, f, y, params.y0 + 1, params.y1, first_year);
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
            clear_year(
                y0,
                params.same_sex_boost,
                &f,
                &m,
                regions,
                &mut blocks,
                &mut pools,
                &mut births,
                first_year,
                params.y1,
                key,
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
            for (&mother, &(u, nu)) in &births[(t - first_year) as usize] {
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
                });
                pools.push(Pool::one(CohortPool::new(
                    females,
                    size - females,
                    [females as f64, (size - females) as f64],
                )));
                add_nonunion_births(&mut births, idx, females, t, t + 1, params.y1, first_year);
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
            clear_year(
                t,
                params.same_sex_boost,
                &f,
                &m,
                regions,
                &mut blocks,
                &mut pools,
                &mut births,
                first_year,
                params.y1,
                key,
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
            );

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
    births: &mut [FxHashMap<u32, (u64, u64)>],
    first_year: i32,
    regions: usize,
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
            add_nonunion_births(births, b as u32, f, year, t + 1, params.y1, first_year);
        }
        // Arrival cells, and the plans' births: in-world later, or abroad
        // (children arriving now).
        let mut cells: BTreeMap<(u32, u32), u64> = BTreeMap::new();
        for &(aw, am, n) in &pairs {
            *cells
                .entry((block_of(t - aw, r) as u32, block_of(t - am, r) as u32))
                .or_default() += n;
        }
        let mut by_f: BTreeMap<u32, Vec<(u32, u64)>> = BTreeMap::new();
        let mut by_m: BTreeMap<u32, Vec<(u32, u64)>> = BTreeMap::new();
        for (&(bf, bm), &n) in &cells {
            by_f.entry(bf).or_default().push((bm, n));
            by_m.entry(bm).or_default().push((bf, n));
        }
        let mut kids: BTreeMap<usize, Vec<MotherShare>> = BTreeMap::new();
        for (bf, partners) in by_f {
            let n: u64 = partners.iter().map(|p| p.1).sum();
            let cohort = cohort_of[&(bf as usize)];
            blocks[bf as usize].union_f.push(UnionCell {
                year: t,
                total: n,
                partners,
                cohorts: vec![(cohort, n)],
                kind: CellKind::Arrival,
            });
            let age = t - blocks[bf as usize].year;
            for leaf in arrival_plans(n, t, age) {
                let (in_world, abroad) = arrival_births(&leaf, age);
                let count = leaf.plan.count;
                for o in bits(in_world) {
                    let year = t + o;
                    if year <= params.y1 {
                        births[(year - first_year) as usize]
                            .entry(bf)
                            .or_default()
                            .0 += count;
                    }
                }
                for c in bits(abroad) {
                    let list = kids.entry(block_of(t - c, r)).or_default();
                    match list.iter_mut().find(|m| m.mother == bf) {
                        Some(m) => m.union_births += count,
                        None => list.push(MotherShare {
                            mother: bf,
                            union_births: count,
                            nonunion_births: 0,
                        }),
                    }
                }
            }
        }
        for (bm, partners) in by_m {
            let n: u64 = partners.iter().map(|p| p.1).sum();
            let cohort = cohort_of[&(bm as usize)];
            blocks[bm as usize].union_m.push(UnionCell {
                year: t,
                total: n,
                partners,
                cohorts: vec![(cohort, n)],
                kind: CellKind::Arrival,
            });
        }
        // The children's cohorts: one per child block.
        for (b, mut mothers) in kids {
            mothers.sort_by_key(|m| m.mother);
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
            add_nonunion_births(births, b as u32, f, year, t + 1, params.y1, first_year);
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
    births: &mut [FxHashMap<u32, (u64, u64)>],
    mother: u32,
    females: u64,
    block_year: i32,
    from: i32,
    to: i32,
    first_year: i32,
) {
    for leaf in nonunion_plans(females, block_year) {
        for k in 0..leaf.births as usize {
            let year = block_year + leaf.ages[k] as i32;
            if (from..=to).contains(&year) {
                births[(year - first_year) as usize]
                    .entry(mother)
                    .or_default()
                    .1 += leaf.count;
            }
        }
    }
}

/// One side of a market: which market's takings, of which sex.
#[derive(Clone, Copy)]
struct Side {
    market: CellKind,
    sex: Sex,
}

/// Clear one year's markets and record the merged cells.
///
/// `want_f[b]` / `want_m[b]` are block `b`'s desired first unions this year
/// (`None`: not in the market). A share `same_sex_share(t)` of each goes to
/// its sex's same-sex market; of the rest, `national_market_share(t)` goes
/// to the national opposite-sex market and the remainder to the block's
/// regional one. Regional markets clear first, then the national one, then
/// the same-sex ones, each against what the earlier ones left. Each market's
/// integer matrix is summed per (row block, column block) and recorded as
/// one cell per block, sex and kind, so a block has at most one cell of each
/// kind per year.
#[allow(clippy::too_many_arguments)]
fn clear_year(
    t: i32,
    same_sex_boost: f64,
    want_f: &[Option<f64>],
    want_m: &[Option<f64>],
    regions: usize,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut [FxHashMap<u32, (u64, u64)>],
    first_year: i32,
    y1: i32,
    key: Key,
) {
    let sigma = (same_sex_share(t) * same_sex_boost).min(0.5);
    let rho = if regions > 1 {
        national_market_share(t)
    } else {
        0.0
    };
    let parts = |want: &[Option<f64>], market: Option<u16>, share: f64| -> Vec<(usize, f64)> {
        want.iter()
            .enumerate()
            .filter_map(|(b, w)| {
                let w = (*w)? * share;
                let here = market.map_or(true, |r| blocks[b].region == r);
                (here && w > 0.0).then_some((b, w))
            })
            .collect()
    };
    let women = Side {
        market: CellKind::InWorld,
        sex: Sex::Female,
    };
    let men = Side {
        market: CellKind::InWorld,
        sex: Sex::Male,
    };
    // Every market's participants, before any cell is recorded.
    let local = (1.0 - sigma) * (1.0 - rho);
    let national = (1.0 - sigma) * rho;
    let regional: Vec<_> = (0..regions as u16)
        .map(|r| (parts(want_f, Some(r), local), parts(want_m, Some(r), local)))
        .collect();
    let nationwide = (parts(want_f, None, national), parts(want_m, None, national));
    let halves = [
        (Sex::Female, parts(want_f, None, sigma / 2.0)),
        (Sex::Male, parts(want_m, None, sigma / 2.0)),
    ];

    let mut opposite: FxHashMap<(u32, u32), u64> = FxHashMap::default();
    for (r, (f, m)) in regional.iter().enumerate() {
        let mk = r as u64;
        clear_market(
            t,
            mk,
            (f, women),
            (m, men),
            age_gap_weight,
            blocks,
            pools,
            key,
            &mut opposite,
        );
    }
    if rho > 0.0 {
        let (f, m) = &nationwide;
        let mk = regions as u64;
        clear_market(
            t,
            mk,
            (f, women),
            (m, men),
            age_gap_weight,
            blocks,
            pools,
            key,
            &mut opposite,
        );
    }
    defer_isolated(&mut opposite, pools, women, men);
    record(
        t, &opposite, women, men, blocks, pools, births, first_year, y1,
    );

    // Same-sex markets: each sex's seekers split into a left and a right
    // half, paired like women and men.
    for (sex, half) in halves {
        let left = Side {
            market: CellKind::SameLeft,
            sex,
        };
        let right = Side {
            market: CellKind::SameRight,
            sex,
        };
        let mut pairs = FxHashMap::default();
        let mk = regions as u64 + 1 + sex as u64;
        clear_market(
            t,
            mk,
            (&half, left),
            (&half, right),
            same_sex_gap_weight,
            blocks,
            pools,
            key,
            &mut pairs,
        );
        defer_isolated(&mut pairs, pools, left, right);
        record(
            t, &pairs, left, right, blocks, pools, births, first_year, y1,
        );
    }
}

/// Defer isolated couples: a row member alone in their cell with a column
/// member alone in theirs. No count-preserving swap can repair such a
/// couple if they are kin, so the union is not formed this year; both stay
/// in their pools for the next market (R1 plan, kin repair). Removing it
/// empties both cells, so no new isolated couple appears.
fn defer_isolated(
    pairs: &mut FxHashMap<(u32, u32), u64>,
    pools: &mut [Pool],
    rows: Side,
    cols: Side,
) {
    let mut per_row: FxHashMap<u32, u64> = FxHashMap::default();
    let mut per_col: FxHashMap<u32, u64> = FxHashMap::default();
    for (&(a, b), &n) in pairs.iter() {
        *per_row.entry(a).or_default() += n;
        *per_col.entry(b).or_default() += n;
    }
    pairs.retain(|&(a, b), &mut n| {
        let isolated = n == 1 && per_row[&a] == 1 && per_col[&b] == 1;
        if isolated {
            pools[a as usize].taken[rows.market as usize][rows.sex as usize] -= 1;
            pools[b as usize].taken[cols.market as usize][cols.sex as usize] -= 1;
        }
        !isolated
    });
}

/// Split each block's takings in one market over its entry cohorts and
/// record the cells on both sides; for opposite-sex markets, also the
/// women's plan births.
#[allow(clippy::too_many_arguments)]
fn record(
    t: i32,
    pairs: &FxHashMap<(u32, u32), u64>,
    rows: Side,
    cols: Side,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut [FxHashMap<u32, (u64, u64)>],
    first_year: i32,
    y1: i32,
) {
    let mut by_row: BTreeMap<u32, Vec<(u32, u64)>> = BTreeMap::new();
    let mut by_col: BTreeMap<u32, Vec<(u32, u64)>> = BTreeMap::new();
    for (&(a, b), &n) in pairs {
        by_row.entry(a).or_default().push((b, n));
        by_col.entry(b).or_default().push((a, n));
    }
    for (side, grouped) in [(rows, by_row), (cols, by_col)] {
        for (b, mut partners) in grouped {
            partners.sort_unstable();
            let n: u64 = partners.iter().map(|p| p.1).sum();
            let cohorts = pools[b as usize].settle(side.market, side.sex);
            debug_assert_eq!(cohorts.iter().map(|c| c.1).sum::<u64>(), n);
            let block = &mut blocks[b as usize];
            let cells = match side.sex {
                Sex::Female => &mut block.union_f,
                Sex::Male => &mut block.union_m,
            };
            cells.push(UnionCell {
                year: t,
                total: n,
                partners,
                cohorts,
                kind: side.market,
            });
            // Opposite-sex women's plans give births; same-sex unions have
            // none in R1.
            if side.market == CellKind::InWorld && side.sex == Sex::Female {
                let mother_age = t - block.year;
                for leaf in union_plans(n, t) {
                    let (offs, nb) = leaf_births(&leaf, mother_age);
                    for &o in &offs[..nb] {
                        let year = t + o as i32;
                        if year <= y1 {
                            births[(year - first_year) as usize].entry(b).or_default().0 +=
                                leaf.count;
                        }
                    }
                }
            }
        }
    }
}

/// Clear one market: two-sided totals, IPF over the age-gap kernel `gap`
/// (of `column's age − row's age`), keyed rounding, capacity caps. Adds the
/// integer matrix to `pairs` and records the takings in the pools. Rows'
/// takings are counted before the columns are capped, so a market whose two
/// sides share a pool (same-sex) never over-draws it.
#[allow(clippy::too_many_arguments)]
fn clear_market(
    t: i32,
    market: u64,
    (parts_f, row_side): (&[(usize, f64)], Side),
    (parts_m, col_side): (&[(usize, f64)], Side),
    gap: fn(i32) -> f64,
    blocks: &[Block],
    pools: &mut [Pool],
    key: Key,
    pairs: &mut FxHashMap<(u32, u32), u64>,
) {
    let (rows, cols) = (parts_f.len(), parts_m.len());
    if rows == 0 || cols == 0 {
        return;
    }
    let (sf, sm): (f64, f64) = (
        parts_f.iter().map(|p| p.1).sum(),
        parts_m.iter().map(|p| p.1).sum(),
    );
    if sf <= 0.0 || sm <= 0.0 {
        return;
    }
    let total = 2.0 * sf * sm / (sf + sm);
    let r: Vec<f64> = parts_f.iter().map(|p| p.1 * total / sf).collect();
    let c: Vec<f64> = parts_m.iter().map(|p| p.1 * total / sm).collect();
    // The kernel depends only on the birth-year gap, so the IPF runs over
    // birth years and each block takes its share of its year's margin: the
    // fixed point `a_i b_j g(y_i - y_j)` has `a_i ∝ r_i` within a year, so
    // `x_ij = X[y_i][y_j] · r_i / R[y_i] · c_j / C[y_j]` exactly (R1 plan,
    // R1b-1). A one-region market has one block per year and is unchanged.
    let group = |parts: &[(usize, f64)], margin: &[f64]| -> (Vec<i32>, Vec<usize>, Vec<f64>) {
        let mut years: Vec<i32> = parts.iter().map(|&(b, _)| blocks[b].year).collect();
        years.sort_unstable();
        years.dedup();
        let idx: Vec<usize> = parts
            .iter()
            .map(|&(b, _)| years.binary_search(&blocks[b].year).unwrap())
            .collect();
        let mut sums = vec![0.0; years.len()];
        for (&y, &v) in idx.iter().zip(margin) {
            sums[y] += v;
        }
        (years, idx, sums)
    };
    let (years_f, yi, rs) = group(parts_f, &r);
    let (years_m, yj, cs) = group(parts_m, &c);
    let (gr, gc) = (years_f.len(), years_m.len());
    let mut g = vec![0.0; gr * gc];
    for (a, &yf) in years_f.iter().enumerate() {
        for (b, &ym) in years_m.iter().enumerate() {
            // age_m - age_f = year_f - year_m
            g[a * gc + b] = gap(yf - ym);
        }
    }
    ipf(&mut g, gr, gc, &rs, &cs);
    let share = |v: f64, sum: f64| if sum > 0.0 { v / sum } else { 0.0 };
    let mut k = vec![0.0; rows * cols];
    for i in 0..rows {
        for j in 0..cols {
            k[i * cols + j] =
                g[yi[i] * gc + yj[j]] * share(r[i], rs[yi[i]]) * share(c[j], cs[yj[j]]);
        }
    }

    // Keyed rounding: floor plus a Bernoulli on the fraction.
    let key = key.with2(t as u64, market);
    let mut x = vec![0u64; rows * cols];
    for (i, &(bf, _)) in parts_f.iter().enumerate() {
        for (j, &(bm, _)) in parts_m.iter().enumerate() {
            let v = k[i * cols + j];
            let fl = v.floor();
            let up = key.with2(bf as u64, bm as u64).unit() < v - fl;
            x[i * cols + j] = fl as u64 + up as u64;
        }
    }
    // Caps: never more unions than expected never-partnered survivors, and
    // never more than the block's unused members. Rows first; their takings
    // count against the columns' caps when both sides share a pool.
    let cap =
        |pool: &Pool, sex: Sex| (pool.never(sex).floor().max(0.0) as u64).min(pool.unused(sex));
    let mut row_sum = vec![0u64; rows];
    for (i, &(bf, _)) in parts_f.iter().enumerate() {
        let row = &mut x[i * cols..(i + 1) * cols];
        trim(row, cap(&pools[bf], row_side.sex));
        row_sum[i] = row.iter().sum();
        pools[bf].taken[row_side.market as usize][row_side.sex as usize] += row_sum[i];
    }
    for (j, &(bm, _)) in parts_m.iter().enumerate() {
        let mut col: Vec<u64> = (0..rows).map(|i| x[i * cols + j]).collect();
        trim(&mut col, cap(&pools[bm], col_side.sex));
        for (i, v) in col.into_iter().enumerate() {
            let cut = x[i * cols + j] - v;
            if cut > 0 {
                // A column trim lowers a row's takings too.
                x[i * cols + j] = v;
                row_sum[i] -= cut;
                let bf = parts_f[i].0;
                pools[bf].taken[row_side.market as usize][row_side.sex as usize] -= cut;
            }
        }
        let n: u64 = (0..rows).map(|i| x[i * cols + j]).sum();
        pools[bm].taken[col_side.market as usize][col_side.sex as usize] += n;
    }

    // Add the matrix to the market's pairs.
    for (i, &(bf, _)) in parts_f.iter().enumerate() {
        for (j, &(bm, _)) in parts_m.iter().enumerate() {
            let n = x[i * cols + j];
            if n > 0 {
                *pairs.entry((bf as u32, bm as u32)).or_default() += n;
            }
        }
    }
}

/// Reduce `cells` until they sum to at most `cap`, taking from the largest
/// cell first (ties: lowest index).
fn trim(cells: &mut [u64], cap: u64) {
    let mut sum: u64 = cells.iter().sum();
    while sum > cap {
        let (i, _) = cells
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
            .unwrap();
        let cut = (sum - cap).min(cells[i]).max(1);
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
            for (s, cells) in [&b.union_f, &b.union_m].into_iter().enumerate() {
                for c in cells {
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
        }
        // Union cells match across sides: each left-role cell's slices
        // equal the mirror slices of the right-role cells, per (year, kind).
        type Side = FxHashMap<(i32, CellKind, u32, u32), u64>;
        let (mut left, mut right): (Side, Side) = Default::default();
        for (bi, b) in l.blocks.iter().enumerate() {
            let bi = bi as u32;
            for (sex, cells) in [(Sex::Female, &b.union_f), (Sex::Male, &b.union_m)] {
                for c in cells {
                    assert_eq!(c.total, c.partners.iter().map(|p| p.1).sum::<u64>());
                    let is_left = match c.kind {
                        CellKind::InWorld | CellKind::Arrival => sex == Sex::Female,
                        CellKind::SameLeft => true,
                        CellKind::SameRight => false,
                    };
                    let kind = if is_left { c.kind } else { c.kind.partner() };
                    for &(pb, n) in &c.partners {
                        if is_left {
                            left.insert((c.year, kind, bi, pb), n);
                        } else {
                            right.insert((c.year, kind, pb, bi), n);
                        }
                    }
                }
            }
            assert!(b.union_f.iter().map(|c| c.total).sum::<u64>() <= b.females);
            assert!(b.union_m.iter().map(|c| c.total).sum::<u64>() <= b.size - b.females);
            // Cells are in (year, kind) order, one per kind and year.
            for cells in [&b.union_f, &b.union_m] {
                assert!(cells
                    .windows(2)
                    .all(|w| (w[0].year, w[0].kind) < (w[1].year, w[1].kind)));
            }
        }
        assert_eq!(left, right);
        assert!(
            left.keys().any(|k| k.1 == CellKind::SameLeft),
            "no same-sex unions"
        );
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
    fn trim_respects_cap() {
        let mut v = vec![5, 9, 1, 9];
        trim(&mut v, 10);
        assert_eq!(v.iter().sum::<u64>(), 10);
        let mut v = vec![1, 2];
        trim(&mut v, 10);
        assert_eq!(v, vec![1, 2]);
    }
}
