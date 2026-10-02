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
//! Blocks are `(birth year, lineage group)`, a group being a region and a
//! heritage (N1). Each year every block's desired unions split between an
//! open market across all groups, its heritage's national market and its
//! group's local market (R1 plan, R1b-1; N1 plan §3); the markets' integer
//! matrices are merged into one union cell per block, sex and year.
//!
//! Spec §5.2; R1 plan decisions D-R1.1–D-R1.5.

use procedural_core::bits::ones;
use procedural_core::fit::{GroupedIpf, IpfStop};
use procedural_core::key::{label, Key};
use procedural_core::life::{cumulative_incidence, first_event_pmf, stable_age_weight, survival};
use procedural_core::partition::{
    apportion_largest_remainder, apportion_largest_remainder_capped, contingency_systematic,
    round_unbiased, sweep_capped, trim_largest_first, SparseCounts, SystematicShares,
};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use std::collections::BTreeMap;

use crate::params::{
    Heritage, MortalityFactor, Params, Repartnering, Sex, Unions, MAX_AGE, MAX_ARRIVAL_AGE,
    MAX_CLASS, MAX_REMARRIAGE_AGE,
};
use crate::plan::{
    arrival_births, arrival_plans, leaf_births, nonunion_plans, union_age_density, PlanLeaf,
    PlanTables,
};

pub use crate::params::MIN_UNION_AGE;
/// Smallest expected number of unions for a block to join a market: below
/// it a seeker group adds nothing but matrix size. Numerical, not a rule
/// about people (the first-union hazard has no upper age).
const MIN_WANT: f64 = 1e-6;

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
    /// The area the couple lives in when the union starts: the market's
    /// (area mode; 0 otherwise).
    pub area: u16,
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
    /// members are in their first union and their partners re-partner
    /// after a divorce (R1c; any later union, R1d).
    FirstWithSecond = 4,
    /// The mirror: this cell's members re-partner after a divorce (their
    /// second union or a later one), their partners in their first.
    SecondWithFirst = 5,
    /// Both partners re-partner after a divorce.
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

    /// True if this cell's members re-partner after a divorce: their second
    /// union or a later one. Their members come from divorced sources.
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

    /// True if the couple feeds the divorced pools when it separates: any
    /// opposite-sex union, whatever its order (R1d: no cap on the number of
    /// unions). Same-sex couples don't re-partner yet.
    pub fn feeds_divorced(self) -> bool {
        !self.same_sex()
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
    /// Natives' migration class (area mode, spec `2026-10-01-ledger-areas.md`):
    /// `(destination area, move year)`. Its members live in the block's area
    /// until the move year and in the destination from then (if still in
    /// their first single spell). `None`: the stayers, and arrivals.
    pub class: Option<(u16, i32)>,
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

/// One birth cohort: a lineage group's births in one year.
#[derive(Clone, Debug)]
pub struct Block {
    /// Birth year.
    pub year: i32,
    /// Lineage group: `region * heritages + heritage`.
    pub group: u16,
    /// Lineage region (index into [`Params::regions`]).
    pub region: u16,
    /// Heritage group (N1).
    pub heritage: Heritage,
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
    /// The class-cell's area (area mode; 0 otherwise). Its members stay
    /// in it while divorced and seek partners there.
    pub area: u16,
    /// Members.
    pub members: u64,
    /// Remarriage parts: `(second-union cell year, kind, class, count)`,
    /// in the order they were recorded; the rest never re-partner.
    pub parts: Vec<(i32, CellKind, u8, u64)>,
    /// Area mode: each part's second-union cell area, which is the
    /// union's area and can differ from `area` (a national or open
    /// market). Empty otherwise.
    pub part_areas: Vec<u16>,
}

impl DivSource {
    /// The area of part `k`'s second-union cell.
    pub fn part_area(&self, k: usize) -> u16 {
        self.part_areas.get(k).copied().unwrap_or(self.area)
    }

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
    /// Lineage groups (regions × heritages); block `(year, g)` has index
    /// `(year - first_year) * groups + g`.
    pub groups: usize,
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
    /// Area mode: couple moves, shared with the world (stage 3).
    pub couple_moves: Option<CoupleMoves>,
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
const TAG_AREA: u64 = label("ledger/area");
const TAG_MIGRATION: u64 = label("ledger/migration");
const TAG_ARRIVAL_COUPLES: u64 = label("ledger/arrival-couples");
const TAG_MOVES: u64 = label("ledger/couple-moves");

/// Move bands of the natives' migration classes (area mode): ages
/// `(from, to)` whose single long-move hazards the band sums, and the age at
/// which its class moves.
const MOVE_BANDS: [(i32, i32, i32); 3] = [(18, 22, 20), (23, 29, 26), (30, 49, 38)];

/// Where single people move (area mode): for each origin region and decade,
/// the shares of the other regions (areas), by attractiveness times the
/// distance decay between area centres (`residence.ron` `gravity`).
struct Migration {
    regions: usize,
    first_decade: i32,
    decades: usize,
    shares: Vec<f64>,
}

impl Migration {
    fn new(params: &Params) -> Self {
        let places = crate::residence::Places::from_params(params).expect("area mode needs places");
        let regions = params.region_count();
        let areas = places.count(crate::residence::AREA) as u32;
        let first_decade = params.y0 - params.y0.rem_euclid(10);
        let decades = ((params.y1 - first_decade) / 10 + 1) as usize;
        let g = &params.residence.gravity;
        let mut shares = vec![0.0; decades * regions * regions];
        for d in 0..decades {
            let year = first_decade + 10 * d as i32;
            for a in 0..areas {
                let origin = places.region(a) as usize;
                let row = (d * regions + origin) * regions;
                let mut total = 0.0;
                for b in (0..areas).filter(|&b| b != a) {
                    let miles = procedural_core::geo::haversine_miles(
                        places.centre(crate::residence::AREA, a),
                        places.centre(crate::residence::AREA, b),
                    );
                    let k = procedural_core::curve::piecewise_power(
                        miles,
                        g.flat_miles,
                        &g.segments,
                        g.beyond,
                    );
                    let w = places.weight(crate::residence::AREA, b, year) * k;
                    shares[row + places.region(b) as usize] += w;
                    total += w;
                }
                if total > 0.0 {
                    for x in &mut shares[row..row + regions] {
                        *x /= total;
                    }
                }
            }
        }
        Self {
            regions,
            first_decade,
            decades,
            shares,
        }
    }

    /// Destination shares over regions of a move out of `origin` in `year`.
    fn shares(&self, origin: u16, year: i32) -> &[f64] {
        let d = ((year - self.first_decade) / 10).clamp(0, self.decades as i32 - 1) as usize;
        let row = (d * self.regions + origin as usize) * self.regions;
        &self.shares[row..row + self.regions]
    }

    /// The natives of a block born in `block_year` in region `own`, `f`
    /// women and `m` men, split into migration classes `(destination, move
    /// year, women, men)` (nonzero only), each class moving at its band's
    /// age if still single: keyed systematic apportionment of the chance of
    /// a first single long move in each band (the pack's `long` rates)
    /// times the destination shares. Bands before `after` are skipped (the
    /// founders' pasts). The rest stay.
    #[allow(clippy::too_many_arguments)]
    fn classes(
        &self,
        params: &Params,
        block_year: i32,
        own: u16,
        f: u64,
        m: u64,
        after: i32,
        key: Key,
    ) -> Vec<(u16, i32, u64, u64)> {
        let long = &params.residence.long;
        let mut weights: Vec<f64> = vec![0.0];
        let mut labels: Vec<(u16, i32)> = vec![(own, i32::MAX)];
        let mut stay = 1.0;
        for &(from, to, at) in &MOVE_BANDS {
            let no_move: f64 = (from..=to)
                .map(|a| 1.0 - (long.by_age.at(a) * long.era.at(block_year + a)).min(1.0))
                .product();
            let p = stay * (1.0 - no_move);
            stay -= p;
            let year = block_year + at;
            if year <= after || year > params.y1 || p <= 0.0 {
                continue;
            }
            for (dest, &sh) in self.shares(own, year).iter().enumerate() {
                if sh > 0.0 {
                    weights.push(p * sh);
                    labels.push((dest as u16, year));
                }
            }
        }
        weights[0] = (1.0 - weights[1..].iter().sum::<f64>()).max(0.0);
        let shares = SystematicShares::new(&weights);
        let mut out: BTreeMap<(i32, u16), (u64, u64)> = BTreeMap::new();
        for (sex, n) in [(0usize, f), (1, m)] {
            shares.for_each_part(n, key.with(sex as u64), |i, c| {
                if i > 0 {
                    let e = out.entry((labels[i].1, labels[i].0)).or_default();
                    if sex == 0 {
                        e.0 += c;
                    } else {
                        e.1 += c;
                    }
                }
            });
        }
        out.into_iter()
            .map(|((year, dest), (f, m))| (dest, year, f, m))
            .collect()
    }
}

/// Couple moves between areas (area mode, stage 3; spec
/// `2026-10-01-ledger-areas.md` §8): each women's plan leaf splits into the
/// couples who stay in the union's area and those who move once, `k`
/// calendar years after the union year (while the union lasts), to another
/// area. The chance of a move in a year is the pack's long-move rate at the
/// woman's age that year; destinations follow [`Migration`]'s shares around
/// the origin. The ledger and the world split leaves alike, so both see the
/// same births by area.
#[derive(Clone)]
pub struct CoupleMoves {
    y0: i32,
    y1: i32,
    /// The long-move rate by `(year - y0, age)`.
    rate: Vec<f64>,
    ages: usize,
    first_decade: i32,
    decades: usize,
    regions: usize,
    /// Destination shares per `(decade, origin)`; `None` if there is
    /// nowhere else to go.
    dests: Vec<Option<SystematicShares>>,
}

impl std::fmt::Debug for CoupleMoves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CoupleMoves({} regions, {}–{})", self.regions, self.y0, self.y1)
    }
}

/// A cell's move offsets: shares over `0` (stays) and `1..=kmax`.
pub struct MoveOffsets(Option<SystematicShares>);

impl CoupleMoves {
    fn new(params: &Params, migration: &Migration) -> Self {
        let (y0, y1) = (params.y0, params.y1);
        let ages = MAX_AGE as usize + 1;
        let long = &params.residence.long;
        let mut rate = Vec::with_capacity((y1 - y0 + 1) as usize * ages);
        for y in y0..=y1 {
            for a in 0..ages as i32 {
                rate.push((long.by_age.at(a) * long.era.at(y)).clamp(0.0, 1.0));
            }
        }
        let dests = (0..migration.decades)
            .flat_map(|d| {
                let year = migration.first_decade + 10 * d as i32;
                (0..migration.regions).map(move |o| (o, year))
            })
            .map(|(o, year)| {
                let w = migration.shares(o as u16, year);
                w.iter().any(|&x| x > 0.0).then(|| SystematicShares::new(w))
            })
            .collect();
        Self {
            y0,
            y1,
            rate,
            ages,
            first_decade: migration.first_decade,
            decades: migration.decades,
            regions: migration.regions,
            dests,
        }
    }

    /// The offsets of a cell's moves: unions begun in `union_year` by women
    /// aged `age`, separating `cutoff` years after it (`None`: never). A move
    /// falls in `1..=kmax`: after the union year, before the separation
    /// year, by the world's last year and the last age.
    pub fn offsets(&self, union_year: i32, age: i32, cutoff: Option<i32>) -> MoveOffsets {
        let kmax = (self.y1 - union_year)
            .min(MAX_AGE as i32 - age)
            .min(cutoff.map_or(i32::MAX, |c| c - 1))
            .min(u8::MAX as i32);
        if kmax < 1 {
            return MoveOffsets(None);
        }
        let pmf = first_event_pmf(kmax - 1, |j| {
            let (y, a) = (union_year + 1 + j, (age + 1 + j).max(0) as usize);
            self.rate[(y - self.y0) as usize * self.ages + a]
        });
        let moved: f64 = pmf.iter().sum();
        if moved <= 0.0 {
            return MoveOffsets(None);
        }
        let mut w = Vec::with_capacity(pmf.len() + 1);
        w.push((1.0 - moved).max(0.0));
        w.extend(pmf);
        MoveOffsets(Some(SystematicShares::new(&w)))
    }

    /// Split `n` couples of a leaf over moves: `f(k, dest, count)`, with
    /// `k = 0` (and `dest = origin`) for those who stay; in part order.
    pub fn split(
        &self,
        offsets: &MoveOffsets,
        n: u64,
        union_year: i32,
        origin: u16,
        key: Key,
        mut f: impl FnMut(u8, u16, u64),
    ) {
        let Some(shares) = &offsets.0 else {
            f(0, origin, n);
            return;
        };
        shares.for_each_part(n, key, |k, c| {
            if k == 0 {
                return f(0, origin, c);
            }
            let year = union_year + k as i32;
            let d = ((year - self.first_decade) / 10).clamp(0, self.decades as i32 - 1) as usize;
            match &self.dests[d * self.regions + origin as usize] {
                Some(dests) => dests.for_each_part(c, key.with(k as u64), |dest, cd| {
                    f(k as u8, dest as u16, cd)
                }),
                None => f(0, origin, c),
            }
        });
    }
}

/// `k` keyed by `area` in area mode (`on`), unchanged otherwise, so worlds
/// without areas keep their keys.
pub fn area_key(k: Key, area: u16, on: bool) -> Key {
    if on {
        k.with2(TAG_AREA, area as u64)
    } else {
        k
    }
}

/// Births by year and mother block: `(union, non-union)`, year-major over
/// every block the ledger will have; births in another area than the
/// mother block's own (area mode) are kept apart, by area.
struct Births {
    first_year: i32,
    blocks: usize,
    counts: Vec<(u64, u64)>,
    other: BTreeMap<(i32, u32, u16), (u64, u64)>,
}

impl Births {
    fn new(first_year: i32, years: usize, blocks: usize) -> Self {
        Self {
            first_year,
            blocks,
            counts: vec![(0, 0); years * blocks],
            other: BTreeMap::new(),
        }
    }

    /// Births of `year` to mother block `mother` in `area`, where `own` is
    /// the mother block's own area.
    fn in_area(&mut self, year: i32, mother: u32, area: u16, own: u16) -> &mut (u64, u64) {
        if area == own {
            self.at(year, mother)
        } else {
            self.other.entry((year, mother, area)).or_default()
        }
    }

    /// Year `year`'s births in other areas than the mothers' own:
    /// `(mother, area, counts)`.
    fn others(&self, year: i32) -> impl Iterator<Item = (u32, u16, &(u64, u64))> {
        self.other
            .range((year, 0, 0)..(year + 1, 0, 0))
            .map(|(&(_, m, a), c)| (m, a, c))
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
    params: &'a Params,
    /// Area mode: couple moves (stage 3).
    moves: Option<&'a CoupleMoves>,
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
    area: u16,
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
                area,
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
fn period_survival(p: &Params, f: MortalityFactor, sex: Sex, age: i32, year: i32) -> f64 {
    survival(age, |a| p.mortality.death_prob_scaled(sex, a, year, f))
}

/// Probability of never having partnered by `age` under `year`'s hazards.
fn never_partnered_share(p: &Params, sex: Sex, age: i32, year: i32) -> f64 {
    survival(age, |a| p.unions.first_union_hazard(sex, a, year))
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
    /// An arrival cohort (immigrants), not natives.
    arrival: bool,
    /// A migration class (area mode): in `dest` from year `moves`
    /// (`i32::MAX`: never moves).
    dest: u16,
    moves: i32,
}

impl CohortPool {
    fn new(f: u64, m: u64, never: [f64; 2]) -> Self {
        Self {
            never,
            alive: [f as f64, m as f64],
            size: [f, m],
            used: [0, 0],
            arrival: false,
            dest: 0,
            moves: i32::MAX,
        }
    }

    /// The area the cohort's never-partnered live in during year `t`, for
    /// a block whose own area is `own`.
    fn area(&self, own: u16, t: i32) -> u16 {
        if t >= self.moves {
            self.dest
        } else {
            own
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
    /// Add source `source` (its index in the block's list) of `members` who
    /// separate in `divorce_year`.
    fn add(&mut self, divorce_year: i32, members: u64, source: u32) {
        let scaled = members as f64 / self.factor;
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
    /// The block's own area (its region in area mode; 0 otherwise, when
    /// every pool and seeker is in area 0).
    own: u16,
    /// Divorced pools per area (each source in its cell's area), by sex.
    div: [Vec<(u16, DivPools)>; 2],
    /// This year's unions taken, per area, by cell kind and sex, not yet
    /// split over the cohorts or sources.
    taken: Vec<(u16, [[u64; 2]; CellKind::COUNT])>,
    /// This year's partnering rate of the never-partnered, by sex:
    /// `[natives, arrival cohorts]` (R1d).
    rate: [[f64; 2]; 2],
    /// A founder block during the first year's market, which pairs the
    /// founders already partnered: its capacity is the expected partnered
    /// members, and the unions it takes leave the never-partnered alone.
    founding: bool,
    /// Wants carried to next year from couples deferred as isolated, per
    /// area, by sex and status (`[never partnered, divorced]`).
    carry: Vec<(u16, [[f64; 2]; 2])>,
    /// This year's never-partnered availability per area ([`Self::refresh`]),
    /// kept current by `settle`, so the markets' caps need no cohort scan.
    avail: Vec<Avail>,
    /// This year's cohorts by area: each [`Avail`]'s `cohorts` range indexes
    /// it, in cohort order.
    area_cohorts: Vec<u32>,
}

impl Avail {
    fn empty(area: u16) -> Self {
        Self {
            area,
            cohorts: (0, 0),
            never: [0.0; 2],
            unused: [0; 2],
            partnered: [0.0; 2],
            want: [0.0; 2],
        }
    }

    fn add(&mut self, c: &CohortPool, rate: &[[f64; 2]; 2]) {
        for si in 0..2 {
            let never = c.never[si].max(0.0);
            self.never[si] += never;
            self.unused[si] += c.size[si] - c.used[si];
            self.partnered[si] += (c.size[si] - c.used[si]) as f64 - never;
            self.want[si] += never * rate[si][c.arrival as usize];
        }
    }
}

/// A pool's never-partnered availability in one area, by sex, before this
/// year's takings: expected never-partnered members, members not yet in a
/// union, and (founding pools) expected members partnered before the world
/// starts and not yet placed in a union.
#[derive(Clone, Copy, Debug)]
struct Avail {
    area: u16,
    /// The area's cohorts: `area_cohorts[cohorts.0..cohorts.1]`.
    cohorts: (u32, u32),
    never: [f64; 2],
    unused: [u64; 2],
    partnered: [f64; 2],
    /// Desired first unions: each cohort's expected never-partnered times
    /// its rate.
    want: [f64; 2],
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
    fn one(c: CohortPool, own: u16) -> Self {
        Self {
            cohorts: vec![c],
            own,
            div: [Vec::new(), Vec::new()],
            taken: Vec::new(),
            rate: [[1.0; 2]; 2],
            founding: false,
            carry: Vec::new(),
            avail: Vec::new(),
            area_cohorts: Vec::new(),
        }
    }

    /// Recompute this year's cohorts by area and each area's availability
    /// (after this year's rates are set), sorted by area, each area's
    /// cohorts in cohort order.
    fn refresh(&mut self, t: i32) {
        let own = self.own;
        let mut by_area: Vec<(u16, u32)> = self
            .cohorts
            .iter()
            .enumerate()
            .map(|(i, c)| (c.area(own, t), i as u32))
            .collect();
        by_area.sort_unstable();
        self.area_cohorts.clear();
        self.area_cohorts.extend(by_area.iter().map(|e| e.1));
        self.avail.clear();
        let mut start = 0;
        while start < by_area.len() {
            let area = by_area[start].0;
            let end = start + by_area[start..].partition_point(|e| e.0 == area);
            self.avail
                .push(self.avail_of(area, (start as u32, end as u32)));
            start = end;
        }
    }

    /// Availability in `area` from its cohorts (an `area_cohorts` range).
    fn avail_of(&self, area: u16, cohorts: (u32, u32)) -> Avail {
        let mut a = Avail::empty(area);
        a.cohorts = cohorts;
        for &i in &self.area_cohorts[cohorts.0 as usize..cohorts.1 as usize] {
            a.add(&self.cohorts[i as usize], &self.rate);
        }
        a
    }

    fn avail(&self, area: u16) -> Option<&Avail> {
        self.avail
            .binary_search_by_key(&area, |a| a.area)
            .ok()
            .map(|i| &self.avail[i])
    }

    /// The want carried into this year for `area`, sex and status.
    fn carried(&self, area: u16, s: Sex, divorced: bool) -> f64 {
        self.carry
            .iter()
            .find(|e| e.0 == area)
            .map_or(0.0, |e| e.1[s as usize][divorced as usize])
    }

    /// Carry one deferred union's want to next year.
    fn carry_one(&mut self, area: u16, s: Sex, divorced: bool) {
        let i = match self.carry.iter().position(|e| e.0 == area) {
            Some(i) => i,
            None => {
                self.carry.push((area, [[0.0; 2]; 2]));
                self.carry.len() - 1
            }
        };
        self.carry[i].1[s as usize][divorced as usize] += 1.0;
    }

    fn taken(&self, area: u16) -> Option<&[[u64; 2]; CellKind::COUNT]> {
        self.taken
            .binary_search_by_key(&area, |e| e.0)
            .ok()
            .map(|i| &self.taken[i].1)
    }

    fn taken_mut(&mut self, area: u16) -> &mut [[u64; 2]; CellKind::COUNT] {
        let i = match self.taken.binary_search_by_key(&area, |e| e.0) {
            Ok(i) => i,
            Err(i) => {
                self.taken.insert(i, (area, [[0; 2]; CellKind::COUNT]));
                i
            }
        };
        &mut self.taken[i].1
    }

    fn taken_of(&self, kinds: &[CellKind], s: Sex, area: u16) -> u64 {
        self.taken(area).map_or(0, |t| {
            kinds.iter().map(|&k| t[k as usize][s as usize]).sum()
        })
    }

    /// The divorced pools of `area`, by sex.
    fn div_in(&self, s: Sex, area: u16) -> Option<&DivPools> {
        self.div[s as usize]
            .iter()
            .find(|d| d.0 == area)
            .map(|d| &d.1)
    }

    fn div_in_mut(&mut self, s: Sex, area: u16) -> &mut DivPools {
        let v = &mut self.div[s as usize];
        let i = match v.iter().position(|d| d.0 == area) {
            Some(i) => i,
            None => {
                v.push((area, DivPools::default()));
                v.len() - 1
            }
        };
        &mut v[i].1
    }

    /// The areas this block has seekers in this year (after
    /// [`Self::refresh`]): its own, its migration classes' destinations, and
    /// its divorced pools' areas.
    fn areas(&self) -> Vec<u16> {
        let mut out: Vec<u16> = self.avail.iter().map(|a| a.area).collect();
        out.extend(self.div.iter().flatten().map(|d| d.0));
        out.sort_unstable();
        out.dedup();
        out
    }

    /// This year's desired first in-world unions of the never-partnered in
    /// `area`: each cohort's expected members times its rate.
    fn want(&self, s: Sex, area: u16) -> f64 {
        self.avail(area).map_or(0.0, |a| a.want[s as usize])
    }

    /// The yearly partnering rate of the block's natives who are single at
    /// `age`, never partnered (hazard `h`) and divorced together: what a
    /// single arrival of that age is given (R1d). `h` alone when the block
    /// has no single natives.
    fn single_rate(&self, s: Sex, age: i32, t: i32, h: f64, rp: &Repartnering) -> f64 {
        let never: f64 = self
            .cohorts
            .iter()
            .filter(|c| !c.arrival)
            .map(|c| c.never[s as usize].max(0.0))
            .sum();
        let mut divorced = 0.0;
        let mut div_want = 0.0;
        for (area, d) in &self.div[s as usize] {
            divorced += d
                .available(t)
                .iter()
                .map(|y| y.scaled.max(0.0))
                .sum::<f64>()
                * d.factor;
            div_want += self.div_want(s, age, t, rp, *area);
        }
        if never + divorced <= 0.0 {
            return h;
        }
        (h * never + div_want) / (never + divorced)
    }

    /// Expected never-partnered members in `area` left this year.
    fn never(&self, s: Sex, area: u16) -> f64 {
        let n = self.avail(area).map_or(0.0, |a| a.never[s as usize]);
        n - self.taken_of(&NEVER_KINDS, s, area) as f64
    }

    /// A founding pool's expected members in `area` partnered before the
    /// world starts and not yet placed, net of this year's takings.
    fn partnered(&self, s: Sex, area: u16) -> f64 {
        let n = self.avail(area).map_or(0.0, |a| a.partnered[s as usize]);
        n - self.taken_of(&NEVER_KINDS, s, area) as f64
    }

    /// Members in `area` not yet in a union, net of this year's takings.
    fn unused(&self, s: Sex, area: u16) -> u64 {
        let n = self.avail(area).map_or(0, |a| a.unused[s as usize]);
        n - self.taken_of(&NEVER_KINDS, s, area)
    }

    /// Desired second unions this year of the divorced in `area`, aged `age`.
    fn div_want(&self, s: Sex, age: i32, t: i32, rp: &Repartnering, area: u16) -> f64 {
        let base = rp.base(s, age, t);
        if base == 0.0 {
            return 0.0;
        }
        let Some(d) = self.div_in(s, area) else {
            return 0.0;
        };
        d.available(t)
            .iter()
            .map(|y| rp.duration(t - y.year) * y.scaled.max(0.0))
            .sum::<f64>()
            * d.factor
            * base
    }

    /// Most second unions the divorced in `area` can take this year:
    /// expected available survivors (rounded up), and available members,
    /// net of this year's takings.
    fn div_cap(&self, s: Sex, t: i32, area: u16) -> u64 {
        let Some(d) = self.div_in(s, area) else {
            return 0;
        };
        let (mut e, mut n) = (0.0, 0u64);
        for y in d.available(t) {
            e += y.scaled.max(0.0);
            n += y.unused;
        }
        let taken = self.taken_of(&DIVORCED_KINDS, s, area);
        ceil_count(e * d.factor).min(n).saturating_sub(taken)
    }

    /// Most unions of `divorced` or never-partnered seekers in `area` this
    /// year: expected available survivors, rounded up, and never more than
    /// the members available. A seeker's expected unions are well below its
    /// pool's expected survivors (a yearly rate, at most doubled by the
    /// market's scaling), so this cap only stops rounding from overshooting
    /// a small pool; a floor (or any rounding that can fall below the
    /// expectation) would trim small pools' unions systematically.
    fn cap(&self, s: Sex, divorced: bool, t: i32, area: u16) -> u64 {
        if divorced {
            self.div_cap(s, t, area)
        } else if self.founding {
            ceil_count(self.partnered(s, area)).min(self.unused(s, area))
        } else {
            ceil_count(self.never(s, area)).min(self.unused(s, area))
        }
    }

    /// Split this year's takings of one never-partnered kind in `area` over
    /// the cohorts there: by never-partnered pool, capped by each cohort's
    /// unused members. Returns `(cohort, count)`.
    fn settle(&mut self, kind: CellKind, s: Sex, area: u16, t: i32, n: u64) -> Vec<(u32, u64)> {
        let si = s as usize;
        let taken = &mut self.taken_mut(area)[kind as usize][si];
        debug_assert!(*taken >= n, "settling more than was taken");
        *taken -= n;
        if n == 0 {
            return Vec::new();
        }
        let own = self.own;
        let range = self
            .avail(area)
            .map(|a| a.cohorts)
            .expect("a settled area was refreshed");
        let here: Vec<usize> = self.area_cohorts[range.0 as usize..range.1 as usize]
            .iter()
            .map(|&i| i as usize)
            .collect();
        let founding = self.founding;
        let weights: Vec<f64> = here
            .iter()
            .map(|&i| {
                let c = &self.cohorts[i];
                if founding {
                    (c.size[si] - c.used[si]) as f64 - c.never[si].max(0.0)
                } else {
                    c.never[si].max(0.0) * self.rate[si][c.arrival as usize]
                }
            })
            .collect();
        let caps: Vec<u64> = here
            .iter()
            .map(|&i| self.cohorts[i].size[si] - self.cohorts[i].used[si])
            .collect();
        let split = apportion_largest_remainder_capped(n, &weights, &caps);
        let mut out = Vec::new();
        for (&i, &k) in here.iter().zip(&split) {
            if k > 0 {
                let c = &mut self.cohorts[i];
                debug_assert_eq!(c.area(own, t), area);
                c.used[si] += k;
                if !founding {
                    c.never[si] -= k as f64;
                }
                out.push((i as u32, k));
            }
        }
        let fresh = self.avail_of(area, range);
        let i = self
            .avail
            .binary_search_by_key(&area, |a| a.area)
            .expect("a settled area was refreshed");
        self.avail[i] = fresh;
        out
    }

    /// Split this year's takings of one divorced kind over the available
    /// sources: first over divorce years by want (hazard times expected
    /// survivors), then within each year over its sources by expected
    /// survivors, capped by members not yet re-partnered. Each level is a
    /// keyed systematic sweep ([`sweep_capped`]): exact, unbiased, linear and
    /// allocation-free. Returns `(source, count)` in source order.
    fn settle_div(
        &mut self,
        kind: CellKind,
        s: Sex,
        t: i32,
        key: Key,
        rp: &Repartnering,
        area: u16,
        n: u64,
    ) -> Vec<(u32, u64)> {
        let si = s as usize;
        let taken = &mut self.taken_mut(area)[kind as usize][si];
        debug_assert!(*taken >= n, "settling more than was taken");
        *taken -= n;
        if n == 0 {
            return Vec::new();
        }
        let d = self.div_in_mut(s, area);
        let ny = d.available(t).len();
        let mut by_year = vec![0u64; ny];
        {
            let years = &d.years[..ny];
            sweep_capped(
                n,
                ny,
                |yi| {
                    let y = &years[yi];
                    (rp.duration(t - y.year) * y.scaled.max(0.0), y.unused)
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
                sweep_capped(
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

/// Year `t`'s cells were recorded market by market (and area by area);
/// order them by (kind, class, area) after the earlier years' (every lookup
/// searches cells by (year, kind, class, area)).
fn sort_year_cells(blocks: &mut [Block], t: i32) {
    blocks.par_iter_mut().for_each(|b| {
        for cells in [&mut b.union_f, &mut b.union_m] {
            let from = cells.partition_point(|c| c.year < t);
            cells[from..].sort_by_key(|c| (c.kind, c.class, c.area));
        }
    });
}

/// The smallest count at least `x` (0 for a non-positive or NaN `x`).
fn ceil_count(x: f64) -> u64 {
    if x > 0.0 {
        x.ceil() as u64
    } else {
        0
    }
}

/// Register a first-union class-cell that separates as a divorced source
/// of its block (R1c): the ledger's list, and the projection's pool.
fn add_source(block: &mut Block, pool: &mut Pool, s: Sex, cell: &UnionCell) {
    if cell.class == 0 || !cell.kind.feeds_divorced() {
        return;
    }
    let src = DivSource {
        year: cell.year,
        kind: cell.kind,
        class: cell.class,
        area: cell.area,
        members: cell.total,
        parts: Vec::new(),
        part_areas: Vec::new(),
    };
    let index = match s {
        Sex::Female => block.div_f.len(),
        Sex::Male => block.div_m.len(),
    } as u32;
    pool.div_in_mut(s, cell.area)
        .add(src.divorce_year(), cell.total, index);
    match s {
        Sex::Female => block.div_f.push(src),
        Sex::Male => block.div_m.push(src),
    }
}

impl Ledger {
    /// Build the ledger. `seed` only keys the integer rounding of market
    /// matrices, so the same parameters and seed always give the same world.
    pub fn build(params: Params, seed: u64) -> Self {
        let key = Key::from_seed(seed).with(TAG_ROUND);
        let plan_root = Key::from_seed(seed).with(TAG_PLANS);
        // Union years: arriving couples' unions began up to their age at
        // arrival before it; every other union starts in y0..=y1.
        let plan_tables = PlanTables::new(
            params.y0 - MAX_ARRIVAL_AGE,
            params.y1,
            &params.fertility,
            &params.heritage,
        );
        // Area mode: lineage regions are residence areas, and the ledger
        // tracks where people live (spec `2026-10-01-ledger-areas.md`).
        let areas_on = params.places.by_area;
        let migration = areas_on.then(|| Migration::new(&params));
        let couple_moves = migration.as_ref().map(|m| CoupleMoves::new(&params, m));
        let plans = Plans {
            root: plan_root,
            tables: &plan_tables,
            params: &params,
            moves: couple_moves.as_ref(),
        };
        let mut class_moves = 0u64;
        let arrival_density: Vec<Vec<f64>> = (params.y0..=params.y1)
            .map(|t| union_age_density(t, MAX_ARRIVAL_AGE, &params))
            .collect();
        assert!(
            params.region_count() >= 1,
            "a world needs at least one region"
        );
        let groups = params.group_count();
        let first_year = params.y0 - params.founder_max_age;
        let years = (params.y1 - first_year + 1) as usize;
        let male_share = params.male_share_at_birth;
        let mut blocks: Vec<Block> = Vec::with_capacity(years * groups);
        let mut pools: Vec<Pool> = Vec::with_capacity(years * groups);
        let mut births = Births::new(first_year, years, years * groups);
        let weight_sum: f64 = params.regions.iter().map(|r| r.founder_weight).sum();
        let heritages = params.heritage_count();
        let founder_mix = params.floored(&params.heritage.founder_mix(params.y0));
        // The founder mix is the composition of the living at y0, so each
        // group's births are scaled by the base survivors over the group's:
        // groups with higher mortality start from more births (exactly 1 for
        // a group with the base mortality).
        let survivors = |f: MortalityFactor| -> f64 {
            (0..=params.founder_max_age)
                .map(|age| {
                    stable_age_weight(params.founder_growth, age as f64)
                        * ((1.0 - male_share)
                            * period_survival(&params, f, Sex::Female, age, params.y0)
                            + male_share * period_survival(&params, f, Sex::Male, age, params.y0))
                })
                .sum()
        };
        let base_survivors = survivors(MortalityFactor {
            infant: 1.0,
            adult: 1.0,
        });
        let founder_scale: Vec<f64> = (0..heritages)
            .map(|h| {
                let f = params
                    .heritage
                    .mortality_factor(Heritage(h as u8), params.y0);
                base_survivors / survivors(f)
            })
            .collect();

        // Founders: survivors at y0 of a stable population, split by region
        // and heritage.
        for y in first_year..=params.y0 {
            let age = params.y0 - y;
            for g in 0..groups {
                let (region, heritage) = (g / heritages, g % heritages);
                let born = params.founder_births
                    * stable_age_weight(params.founder_growth, age as f64)
                    * params.regions[region].founder_weight
                    / weight_sum
                    * founder_mix[heritage]
                    * founder_scale[heritage];
                let mf = params
                    .heritage
                    .mortality_factor(Heritage(heritage as u8), params.y0);
                let f = (born
                    * (1.0 - male_share)
                    * period_survival(&params, mf, Sex::Female, age, params.y0))
                .round() as u64;
                let m =
                    (born * male_share * period_survival(&params, mf, Sex::Male, age, params.y0))
                        .round() as u64;
                let idx = blocks.len() as u32;
                blocks.push(Block {
                    year: y,
                    group: g as u16,
                    region: region as u16,
                    heritage: Heritage(heritage as u8),
                    founder: true,
                    size: f + m,
                    females: f,
                    cohorts: vec![Cohort {
                        arrival: None,
                        class: None,
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
                pools.push(Pool::one(
                    CohortPool::new(
                        f,
                        m,
                        [
                            f as f64 * never_partnered_share(&params, Sex::Female, age, params.y0),
                            m as f64 * never_partnered_share(&params, Sex::Male, age, params.y0),
                        ],
                    ),
                    if areas_on { region as u16 } else { 0 },
                ));
                settle_natives(
                    idx as usize,
                    &mut blocks,
                    &mut pools,
                    &mut births,
                    migration.as_ref(),
                    &params,
                    params.y0,
                    key,
                );
            }
        }

        // Founder couples already partnered at y0.
        {
            let y0 = params.y0;
            for p in pools.iter_mut() {
                p.founding = true;
                p.refresh(y0);
            }
            let wanting: Vec<Seeker> = (0..blocks.len())
                .filter(|&b| y0 - blocks[b].year >= MIN_UNION_AGE)
                .map(|b| {
                    let own = pools[b].own;
                    let partnered = |sex: Sex| {
                        let n = match sex {
                            Sex::Female => blocks[b].females,
                            Sex::Male => blocks[b].size - blocks[b].females,
                        };
                        Some(n as f64 - pools[b].never(sex, own))
                    };
                    Seeker {
                        block: b as u32,
                        area: own,
                        region: blocks[b].region,
                        never: [partnered(Sex::Female), partnered(Sex::Male)],
                        div: [None, None],
                    }
                })
                .collect();
            clear_year(
                y0,
                &params,
                &wanting,
                &mut blocks,
                &mut pools,
                &mut births,
                params.y1,
                key,
                plans,
                &mut class_moves,
            );
            sort_year_cells(&mut blocks, y0);
            for p in pools.iter_mut() {
                p.founding = false;
            }
        }

        // Expected living, carried from year to year for the inflow's base.
        let mut alive: f64 = pools
            .iter()
            .flat_map(|p| &p.cohorts)
            .map(|c| c.alive[0] + c.alive[1])
            .sum();
        for t in params.y0 + 1..=params.y1 {
            // 1. The cohorts born this year: exactly the births recorded for
            //    them, each in its mother's group.
            let mut by_group: Vec<Vec<MotherShare>> = vec![Vec::new(); groups];
            for (mother, &(u, nu)) in births.year(t) {
                if u + nu == 0 {
                    continue;
                }
                let mother = mother as u32;
                by_group[blocks[mother as usize].group as usize].push(MotherShare {
                    mother,
                    union_births: u,
                    nonunion_births: nu,
                });
            }
            // Births in another area than the mother block's own (area
            // mode): the child's block is the area's, of the mother's
            // heritage.
            for (mother, area, &(u, nu)) in births.others(t) {
                let g = area as usize * heritages + blocks[mother as usize].heritage.index();
                by_group[g].push(MotherShare {
                    mother,
                    union_births: u,
                    nonunion_births: nu,
                });
            }
            for (g, mut mothers) in by_group.into_iter().enumerate() {
                mothers.sort_by_key(|m| m.mother);
                let size: u64 = mothers
                    .iter()
                    .map(|m| m.union_births + m.nonunion_births)
                    .sum();
                let females = apportion_largest_remainder(size, &[1.0 - male_share, male_share])[0];
                let idx = blocks.len() as u32;
                blocks.push(Block {
                    year: t,
                    group: g as u16,
                    region: (g / heritages) as u16,
                    heritage: Heritage((g % heritages) as u8),
                    founder: false,
                    size,
                    females,
                    cohorts: vec![Cohort {
                        arrival: None,
                        class: None,
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
                pools.push(Pool::one(
                    CohortPool::new(
                        females,
                        size - females,
                        [females as f64, (size - females) as f64],
                    ),
                    if areas_on { (g / heritages) as u16 } else { 0 },
                ));
                settle_natives(
                    idx as usize,
                    &mut blocks,
                    &mut pools,
                    &mut births,
                    migration.as_ref(),
                    &params,
                    params.y0,
                    key,
                );
                alive += size as f64;
            }

            // 2. This year's union markets. Natives who never partnered seek
            //    first unions at the first-union hazard. Single immigrants
            //    arrived as they were, with histories the world doesn't
            //    know, so they partner at the rate of the natives of their
            //    age who are single, never partnered and divorced together
            //    (R1d): statistically like the people around them.
            // Each pool is its own task: rates, then this year's availability
            // (blocks too young to partner have neither).
            {
                let blocks: &[Block] = &blocks;
                pools.par_iter_mut().enumerate().for_each(|(b, pool)| {
                    let age = t - blocks[b].year;
                    if age < MIN_UNION_AGE {
                        pool.rate = [[0.0; 2]; 2];
                        return;
                    }
                    for sex in [Sex::Female, Sex::Male] {
                        let h = params.unions.first_union_hazard(sex, age as u32, t);
                        pool.rate[sex as usize] =
                            [h, pool.single_rate(sex, age, t, h, &params.repartnering)];
                    }
                    pool.refresh(t);
                });
            }
            // Each block's seekers, per area it has people in this year (its
            // own, its migration classes' destinations, its divorced pools'
            // areas), in block order.
            let per_block: Vec<Vec<Seeker>> = pools
                .par_iter()
                .enumerate()
                .map(|(b, pool)| {
                    let mut wanting: Vec<Seeker> = Vec::new();
                    let age = t - blocks[b].year;
                    if age < MIN_UNION_AGE {
                        return wanting;
                    }
                    let div_age = (MIN_UNION_AGE..=MAX_REMARRIAGE_AGE).contains(&age);
                    for area in pool.areas() {
                        // Wants deferred last year (couples isolated on both
                        // sides) come back on top: delayed, not lost.
                        let never = [Sex::Female, Sex::Male].map(|sex| {
                            let w = pool.want(sex, area) + pool.carried(area, sex, false);
                            (w > MIN_WANT).then_some(w)
                        });
                        // The divorced's desired second unions (R1c).
                        let div = [Sex::Female, Sex::Male].map(|sex| {
                            if !div_age {
                                return None;
                            }
                            let w = pool.div_want(sex, age, t, &params.repartnering, area)
                                + pool.carried(area, sex, true);
                            (w > MIN_WANT).then_some(w)
                        });
                        if never.iter().chain(&div).any(Option::is_some) {
                            wanting.push(Seeker {
                                block: b as u32,
                                area,
                                region: if areas_on { area } else { blocks[b].region },
                                never,
                                div,
                            });
                        }
                    }
                    wanting
                })
                .collect();
            let wanting: Vec<Seeker> = per_block.into_iter().flatten().collect();
            for pool in pools.iter_mut() {
                pool.carry.clear();
            }
            clear_year(
                t,
                &params,
                &wanting,
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
                groups,
                (key, plans),
                &arrival_density[(t - params.y0) as usize],
            );
            sort_year_cells(&mut blocks, t);

            // 4. Mortality over the year; the survivors are next year's base.
            // Each pool on its own core; the living are then summed in pool
            // order, so the total is the same on any number of cores.
            let living = |b: usize| {
                let age = t - blocks[b].year;
                (0..=MAX_AGE as i32).contains(&age)
            };
            pools.par_iter_mut().enumerate().for_each(|(b, pool)| {
                if !living(b) {
                    return;
                }
                let age = t - blocks[b].year;
                let f = params.heritage.mortality_factor(blocks[b].heritage, t);
                let q = [
                    params
                        .mortality
                        .death_prob_scaled(Sex::Female, age as u32, t, f),
                    params
                        .mortality
                        .death_prob_scaled(Sex::Male, age as u32, t, f),
                ];
                for c in &mut pool.cohorts {
                    for ((never, living), q) in c.never.iter_mut().zip(&mut c.alive).zip(q) {
                        *never *= 1.0 - q;
                        *living *= 1.0 - q;
                    }
                }
                for (ds, q) in pool.div.iter_mut().zip(q) {
                    for (_, d) in ds.iter_mut() {
                        if age > MAX_REMARRIAGE_AGE {
                            // Too old to re-partner: retire every source.
                            d.years.clear();
                            continue;
                        }
                        d.factor *= 1.0 - q;
                        d.retire_years();
                    }
                }
            });
            alive = 0.0;
            for (b, pool) in pools.iter().enumerate() {
                if living(b) {
                    for c in &pool.cohorts {
                        alive += c.alive[0];
                        alive += c.alive[1];
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
            groups,
            blocks,
            base,
            plan_root,
            class_moves,
            arrival_density,
            plan_tables,
            couple_moves,
        }
    }

    /// Total ids.
    pub fn population(&self) -> u64 {
        *self.base.last().unwrap()
    }

    /// Block index for a birth year and lineage group.
    pub fn block_of(&self, year: i32, group: u16) -> Option<u32> {
        let i = year - self.first_year;
        let n_years = (self.blocks.len() / self.groups) as i32;
        ((0..n_years).contains(&i) && (group as usize) < self.groups)
            .then(|| (i as usize * self.groups + group as usize) as u32)
    }

    /// Block indices of every group's cohort born in `year`.
    pub fn blocks_of_year(&self, year: i32) -> std::ops::Range<u32> {
        match self.block_of(year, 0) {
            Some(b) => b..b + self.groups as u32,
            None => 0..0,
        }
    }
}

/// Immigrants arriving in year `t`: per lineage group (region weight times
/// the year's heritage mix of arrivals), the pack's immigration rate of
/// the expected living `alive` (this year's births included), its couple share of them in
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
    groups: usize,
    (key, plans): (Key, Plans),
    dens: &[f64],
) {
    let im = &params.immigration;
    let arriving = (im.rate(t) * alive).round() as u64;
    let heritages = params.heritage_count();
    let mix = params.floored(&params.heritage.immigrant_mix(t));
    let group_w: Vec<f64> = (0..groups)
        .map(|g| params.regions[g / heritages].immigrant_weight * mix[g % heritages])
        .collect();
    let ages: Vec<i32> = (im.min_age..=MAX_ARRIVAL_AGE).collect();
    let age_w: Vec<f64> = ages.iter().map(|&a| im.age_weight(a)).collect();
    let block_of = |year: i32, g: usize| ((year - first_year) as usize) * groups + g;
    // Share ever partnered by each age under this year's schedule, per sex.
    let ever: [Vec<f64>; 2] = [Sex::Female, Sex::Male].map(|sex| {
        cumulative_incidence(MAX_ARRIVAL_AGE, |a| {
            params.unions.first_union_hazard(sex, a as u32, t)
        })
    });
    let partnered = |sex: Sex, age: i32| ever[sex as usize][age as usize];
    for (r, &nr) in apportion_largest_remainder(arriving, &group_w)
        .iter()
        .enumerate()
    {
        let men_share = params
            .heritage
            .immigrant_male_share(Heritage((r % heritages) as u8), t)
            .unwrap_or_else(|| im.male_share(t));
        // `r` is the group; arrivals live in its region (area mode).
        let arrival_area = if params.places.by_area {
            (r / heritages) as u16
        } else {
            0
        };
        // Couples: wives by age, then husbands by the age-gap kernel, never
        // more than `max_husband_younger` years younger.
        // Rounded without bias: a group's few arrivals a year still arrive
        // as couples at the couple share.
        let couples = round_unbiased(
            im.couple_share(t) * nr as f64 / 2.0,
            key.with(TAG_ARRIVAL_COUPLES)
                .with2(t as u64, r as u64)
                .unit(),
        )
        .min(nr / 2);
        let singles = nr - 2 * couples;
        let wife_w: Vec<f64> = ages
            .iter()
            .zip(&age_w)
            .map(|(&a, &w)| {
                if a >= im.min_couple_age {
                    w * partnered(Sex::Female, a)
                } else {
                    0.0
                }
            })
            .collect();
        let mut pairs: Vec<(i32, i32, u64)> = Vec::new();
        for (&aw, &n) in ages
            .iter()
            .zip(&apportion_largest_remainder(couples, &wife_w))
        {
            if n == 0 {
                continue;
            }
            let husband_w: Vec<f64> = ages
                .iter()
                .zip(&age_w)
                .map(|(&am, &w)| {
                    if am - aw >= -im.max_husband_younger {
                        params.unions.age_gap_weight(am - aw) * w * partnered(Sex::Male, am)
                    } else {
                        0.0
                    }
                })
                .collect();
            for (&am, &k) in ages.iter().zip(&apportion_largest_remainder(n, &husband_w)) {
                if k > 0 {
                    pairs.push((aw, am, k));
                }
            }
        }
        // Singles carry the rest of the era's sex ratio.
        let single_men = ((men_share * (2 * couples + singles) as f64).round() as i64
            - couples as i64)
            .clamp(0, singles as i64) as u64;
        let single_f = apportion_largest_remainder(singles - single_men, &age_w);
        let single_m = apportion_largest_remainder(single_men, &age_w);
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
                class: None,
                size: f + m,
                females: f,
                mothers: Vec::new(),
            });
            blocks[b].size += f + m;
            blocks[b].females += f;
            let mut pool = CohortPool::new(f, m, [sf as f64, sm as f64]);
            pool.used = [cf, cm];
            pool.arrival = true;
            pools[b].cohorts.push(pool);
            let year = blocks[b].year;
            add_nonunion_births(
                births,
                b as u32,
                f,
                year,
                t + 1,
                params.y1,
                (params, blocks[b].heritage),
            );
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
        let shares = SystematicShares::new(&params.dissolution.class_pmf(t, false));
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
                arrival_area,
                &mut partners,
                &[(cohort as u32, n)],
                Members::Cohorts,
                ckey.with3(t as u64, bf as u64, 0),
            );
            for cell in cells {
                let pk = area_key(
                    plan_key(plans.root, bf, t, CellKind::Arrival, cell.class),
                    arrival_area,
                    params.places.by_area,
                );
                let mca = params.immigration.min_couple_age;
                let h = blocks[bf as usize].heritage;
                for leaf in arrival_plans(cell.total, t, age, pk, dens, plans.tables, h, mca) {
                    let (in_world, abroad) = arrival_births(&leaf, age, cell.class, params);
                    let count = leaf.plan.count;
                    for o in ones(in_world) {
                        let year = t + o;
                        if year <= params.y1 {
                            births.at(year, bf).0 += count;
                        }
                    }
                    for c in ones(abroad) {
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
                arrival_area,
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
            let f = apportion_largest_remainder(size, &[1.0 - male, male])[0];
            blocks[b].cohorts.push(Cohort {
                arrival: Some(t),
                class: None,
                size,
                females: f,
                mothers,
            });
            blocks[b].size += size;
            blocks[b].females += f;
            let mut pool = CohortPool::new(f, size - f, [f as f64, (size - f) as f64]);
            pool.arrival = true;
            pools[b].cohorts.push(pool);
            let year = blocks[b].year;
            add_nonunion_births(
                births,
                b as u32,
                f,
                year,
                t + 1,
                params.y1,
                (params, blocks[b].heritage),
            );
        }
    }
}

/// Record a block's non-union births in the years `[from, to]`.
fn add_nonunion_births(
    births: &mut Births,
    mother: u32,
    females: u64,
    block_year: i32,
    from: i32,
    to: i32,
    (params, h): (&Params, Heritage),
) {
    add_nonunion_births_in(
        births,
        mother,
        females,
        block_year,
        from,
        to,
        (params, h),
        (0, None),
    );
}

/// Non-union births of a cohort's `females`, in the area the cohort lives
/// in at each birth (`(own, class)`: the block's area, and the cohort's
/// migration class).
#[allow(clippy::too_many_arguments)]
fn add_nonunion_births_in(
    births: &mut Births,
    mother: u32,
    females: u64,
    block_year: i32,
    from: i32,
    to: i32,
    (params, h): (&Params, Heritage),
    (own, class): (u16, Option<(u16, i32)>),
) {
    let factor = params.heritage.fertility_factor(h, block_year + 25);
    for leaf in nonunion_plans(females, block_year, &params.fertility, factor) {
        for k in 0..leaf.births as usize {
            let year = block_year + leaf.ages[k] as i32;
            if (from..=to).contains(&year) {
                let area = match class {
                    Some((dest, moves)) if year >= moves => dest,
                    _ => own,
                };
                births.in_area(year, mother, area, own).1 += leaf.count;
            }
        }
    }
}

/// Area mode: split block `b`'s natives (its only cohort so far) into the
/// stayers and migration classes, in the block and its pool, and record
/// each native cohort's non-union births in its areas. Otherwise just the
/// natives' non-union births.
#[allow(clippy::too_many_arguments)]
fn settle_natives(
    b: usize,
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut Births,
    migration: Option<&Migration>,
    params: &Params,
    after: i32,
    key: Key,
) {
    let block = &mut blocks[b];
    let pool = &mut pools[b];
    let (year, h, own) = (block.year, block.heritage, pool.own);
    if let Some(mig) = migration {
        let natives = block.cohorts[0].clone();
        let (f, m) = (natives.females, natives.size - natives.females);
        let classes = mig.classes(
            params,
            year,
            own,
            f,
            m,
            after,
            key.with2(TAG_MIGRATION, b as u64),
        );
        let base = pool.cohorts[0];
        let scale = |x: f64, part: u64, whole: u64| {
            if whole == 0 {
                0.0
            } else {
                x * part as f64 / whole as f64
            }
        };
        let (mut f0, mut m0) = (f, m);
        for &(dest, moves, cf, cm) in &classes {
            f0 -= cf;
            m0 -= cm;
            block.cohorts.push(Cohort {
                arrival: None,
                class: Some((dest, moves)),
                size: cf + cm,
                females: cf,
                mothers: Vec::new(),
            });
            let mut c = CohortPool::new(
                cf,
                cm,
                [scale(base.never[0], cf, f), scale(base.never[1], cm, m)],
            );
            c.alive = [scale(base.alive[0], cf, f), scale(base.alive[1], cm, m)];
            c.dest = dest;
            c.moves = moves;
            pool.cohorts.push(c);
        }
        block.cohorts[0].size = f0 + m0;
        block.cohorts[0].females = f0;
        let c0 = &mut pool.cohorts[0];
        *c0 = CohortPool {
            never: [scale(base.never[0], f0, f), scale(base.never[1], m0, m)],
            alive: [scale(base.alive[0], f0, f), scale(base.alive[1], m0, m)],
            size: [f0, m0],
            ..base
        };
    }
    let from = (year + 1).max(params.y0 + 1);
    for c in &blocks[b].cohorts {
        add_nonunion_births_in(
            births,
            b as u32,
            c.females,
            year,
            from,
            params.y1,
            (params, h),
            (own, c.class),
        );
    }
}

/// A market seeker: a block's never-partnered or divorced members of one
/// sex (R1c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Who {
    block: u32,
    /// The area the seekers live in (area mode; 0 otherwise).
    area: u16,
    divorced: bool,
}

impl Who {
    /// Key code for rounding: the block, the area and the status.
    fn code(self) -> u64 {
        (self.area as u64) << 33 | (self.block as u64) << 1 | self.divorced as u64
    }
}

/// One block's seekers in one area this year: desired unions by sex
/// (`Sex as usize`), never partnered and divorced.
#[derive(Clone, Copy, Debug)]
struct Seeker {
    block: u32,
    /// The area they live in (area mode; 0 otherwise).
    area: u16,
    /// The region whose markets they join: their area in area mode, the
    /// block's lineage region otherwise.
    region: u16,
    never: [Option<f64>; 2],
    div: [Option<f64>; 2],
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
    fn kernel(self, gap: i32, row: bool, col: bool, u: &Unions) -> f64 {
        match self {
            Market::Opposite => {
                let g = if row || col {
                    u.remarriage_gap_weight(gap)
                } else {
                    u.age_gap_weight(gap)
                };
                g * u.status_affinity(row, col)
            }
            Market::Same(_) => u.same_sex_gap_weight(gap),
        }
    }
}

/// Every market kernel value by birth-year gap: the kernels depend only on
/// the gap and the partners' statuses, so they are tabled once.
struct Kernels {
    /// `[opposite (row divorced, column divorced) × 4, same-sex][gap + GAPS]`.
    values: [Vec<f64>; 5],
}

/// Gaps beyond this many years (never between two living seekers) are
/// computed directly.
const GAPS: i32 = 128;

impl Kernels {
    fn new(u: &Unions) -> Self {
        let table = |f: &dyn Fn(i32) -> f64| (-GAPS..=GAPS).map(f).collect::<Vec<f64>>();
        let opp = |row: bool, col: bool| table(&|gap| Market::Opposite.kernel(gap, row, col, u));
        Self {
            values: [
                opp(false, false),
                opp(false, true),
                opp(true, false),
                opp(true, true),
                table(&|gap| Market::Same(Sex::Female).kernel(gap, false, false, u)),
            ],
        }
    }

    fn at(&self, market: Market, gap: i32, row: bool, col: bool, u: &Unions) -> f64 {
        if gap.abs() > GAPS {
            return market.kernel(gap, row, col, u);
        }
        let k = match market {
            Market::Opposite => (row as usize) << 1 | col as usize,
            Market::Same(_) => 4,
        };
        self.values[k][(gap + GAPS) as usize]
    }
}

/// A market's couples by row and column seeker.
type Pairs = FxHashMap<(Who, Who), u64>;

/// One market's key, kind, row seekers and column seekers.
type MarketSeekers = (u64, Market, Vec<(Who, f64)>, Vec<(Who, f64)>);

/// Clear one year's markets and record the merged cells.
///
/// `want_f[b]` / `want_m[b]` are block `b`'s desired first unions this year
/// (`None`: not in the market), and `div_f` / `div_m` its divorced members'
/// desired re-partnering (R1c). Each block's wants split (N1 plan §3):
/// - a share `same_sex_share(t)` of first-union wants goes to its sex's
///   same-sex markets: the open-market share of it to the sex's open one,
///   the rest to the sex's heritage market;
/// - of the rest, and of all re-partnering wants, the open-market share
///   goes to the open opposite-sex market across every group; of the
///   remainder, `national_market_share(t)` goes to the heritage's national
///   market and the rest to the block's group's local one.
///
/// Every market's matrix is solved at once; caps then apply in order
/// (local, national, open; then same-sex), each against what the earlier
/// ones left. The divorced and the never-partnered meet in the same
/// opposite-sex markets; a couple's cell kinds follow from their statuses.
/// Each sex's markets' couples are summed per (row block, column block,
/// kinds), split into dissolution classes and recorded as one cell per
/// block, sex, kind and class.
#[allow(clippy::too_many_arguments)]
/// The kind of a market a seeker's wants are shared into ([`clear_year`]).
#[derive(Clone, Copy)]
enum Share {
    Local,
    National,
    Open,
    RegionOpen,
    SameHeritage,
    SameOpen,
    SameLocal,
    SameRegion,
}

fn clear_year(
    t: i32,
    params: &Params,
    wanting: &[Seeker],
    blocks: &mut [Block],
    pools: &mut [Pool],
    births: &mut Births,
    y1: i32,
    key: Key,
    plans: Plans,
    class_moves: &mut u64,
) {
    let groups = params.group_count();
    let heritages = params.heritage_count();
    let regions = groups / heritages;
    let sigma = (params.unions.same_sex_share(t) * params.same_sex_boost).min(0.5);
    let rho = if params.region_count() > 1 {
        params.unions.national_market_share(t)
    } else {
        0.0
    };
    // The open-market share by heritage and sex, once for the year.
    let omegas: Vec<[f64; 2]> = (0..heritages)
        .map(|h| {
            [Sex::Female, Sex::Male].map(|sex| {
                (params.heritage.open_market_share(t, Heritage(h as u8), sex)
                    * params.open_market_boost)
                    .min(1.0)
            })
        })
        .collect();
    let omega = |b: usize, sex: Sex| omegas[blocks[b].heritage.index()][sex as usize];
    // The seekers of each market, by index into `wanting` (in block order):
    // by group (region × heritage), by heritage, by region, and everyone.
    let mut of_group: Vec<Vec<usize>> = vec![Vec::new(); groups];
    let mut of_heritage: Vec<Vec<usize>> = vec![Vec::new(); heritages];
    let mut of_region: Vec<Vec<usize>> = vec![Vec::new(); regions];
    for (i, sk) in wanting.iter().enumerate() {
        let h = blocks[sk.block as usize].heritage.index();
        of_group[sk.region as usize * heritages + h].push(i);
        of_heritage[h].push(i);
        of_region[sk.region as usize].push(i);
    }
    let everyone: Vec<usize> = (0..wanting.len()).collect();
    // The opposite-sex share of a status's wants (first unions lose the
    // same-sex share; re-partnering is opposite-sex only).
    let opposite = |divorced: bool| if divorced { 1.0 } else { 1.0 - sigma };
    // With local open markets, the open and same-sex markets split like the
    // heritage ones: a share per region and the national share across.
    let local_open = params.unions.local_open_markets && rho > 0.0;
    let open_local = if local_open { 1.0 - rho } else { 0.0 };
    let (same_local, same_national) = if local_open {
        (1.0 - rho, rho)
    } else {
        (0.0, 1.0)
    };
    // A seeker's share of its wants in a market of kind `s`.
    let share = |s: Share, sex: Sex, b: usize, divorced: bool| -> f64 {
        let w = omega(b, sex);
        match s {
            Share::Local => opposite(divorced) * (1.0 - w) * (1.0 - rho),
            Share::National => opposite(divorced) * (1.0 - w) * rho,
            Share::Open => opposite(divorced) * w * (1.0 - open_local),
            Share::RegionOpen => opposite(divorced) * w * open_local,
            Share::SameHeritage => sigma / 2.0 * (1.0 - w) * same_national,
            Share::SameOpen => sigma / 2.0 * w * same_national,
            Share::SameLocal => sigma / 2.0 * (1.0 - w) * same_local,
            Share::SameRegion => sigma / 2.0 * w * same_local,
        }
    };
    // A market's seekers: the listed seekers, each status's wants times
    // their share; `with_div` adds the divorced.
    let parts = |sex: Sex, members: &[usize], with_div: bool, s: Share| -> Vec<(Who, f64)> {
        let si = sex as usize;
        let mut out = Vec::new();
        for &i in members {
            let sk = &wanting[i];
            let b = sk.block as usize;
            let div = if with_div { sk.div[si] } else { None };
            for (w, divorced) in [(sk.never[si], false), (div, true)] {
                if let Some(w) = w
                    .map(|w| w * share(s, sex, b, divorced))
                    .filter(|&w| w > 0.0)
                {
                    let who = Who {
                        block: sk.block,
                        area: sk.area,
                        divorced,
                    };
                    out.push((who, w));
                }
            }
        }
        out
    };
    // Ids of the markets added by local open markets, after the others.
    let extra = (groups + heritages + 1 + 2 * (heritages + 1)) as u64;
    let (fe, ma) = (Sex::Female, Sex::Male);

    // Every market, in order: its id, kind, members and share.
    let mut defs: Vec<(u64, Market, &[usize], Share)> = Vec::new();
    for (g, members) in of_group.iter().enumerate() {
        defs.push((g as u64, Market::Opposite, members, Share::Local));
    }
    if rho > 0.0 {
        for (h, members) in of_heritage.iter().enumerate() {
            defs.push((
                (groups + h) as u64,
                Market::Opposite,
                members,
                Share::National,
            ));
        }
    }
    defs.push((
        (groups + heritages) as u64,
        Market::Opposite,
        &everyone,
        Share::Open,
    ));
    if local_open {
        for (r, members) in of_region.iter().enumerate() {
            defs.push((
                extra + r as u64,
                Market::Opposite,
                members,
                Share::RegionOpen,
            ));
        }
    }
    let opposite_markets = defs.len();
    // Same-sex: each sex's seekers split into a left and a right half,
    // paired like women and men; per heritage, then one open market. With
    // local open markets, each heritage market splits into a part per
    // region and the national part, and so does the open one.
    let mut same_markets = [0usize; 2];
    for sex in [fe, ma] {
        let before = defs.len();
        let base = (groups + heritages + 1 + sex as usize * (heritages + 1)) as u64;
        let same = Market::Same(sex);
        for (h, members) in of_heritage.iter().enumerate() {
            defs.push((base + h as u64, same, members, Share::SameHeritage));
        }
        defs.push((base + heritages as u64, same, &everyone, Share::SameOpen));
        if local_open {
            let sbase = extra + regions as u64 + sex as u64 * (groups + regions) as u64;
            for (g, members) in of_group.iter().enumerate() {
                defs.push((sbase + g as u64, same, members, Share::SameLocal));
            }
            for (r, members) in of_region.iter().enumerate() {
                defs.push((
                    sbase + (groups + r) as u64,
                    same,
                    members,
                    Share::SameRegion,
                ));
            }
        }
        same_markets[sex as usize] = defs.len() - before;
    }
    // Every market's participants, before any cell is recorded; each is a
    // pure function of the seekers, so they are listed on every core.
    let seekers: Vec<MarketSeekers> = defs
        .par_iter()
        .map(|&(mk, market, members, s)| match market {
            Market::Opposite => (
                mk,
                market,
                parts(fe, members, true, s),
                parts(ma, members, true, s),
            ),
            Market::Same(sex) => {
                let half = parts(sex, members, false, s);
                (mk, market, half.clone(), half)
            }
        })
        .collect();
    let specs: Vec<MarketSpec> = seekers
        .iter()
        .map(|(mk, market, rows, cols)| MarketSpec {
            mk: *mk,
            rows,
            cols,
            market: *market,
        })
        .collect();

    // Every market's matrix depends only on its participants (fixed above)
    // and the blocks' birth years, so all of them are solved at once; the
    // caps then apply in order, since they depend on earlier takings.
    let kernels = Kernels::new(&params.unions);
    let solved: Vec<SparseCounts> = {
        let blocks: &[Block] = blocks;
        specs
            .par_iter()
            .map(|spec| solve_market(t, spec, blocks, key, &params.unions, &kernels))
            .collect()
    };
    let mut solved = specs.iter().zip(solved);

    let mut opposite_pairs = Pairs::default();
    for (spec, x) in solved.by_ref().take(opposite_markets) {
        apply_market(t, spec, x, pools, &mut opposite_pairs);
    }
    defer_isolated(&mut opposite_pairs, Market::Opposite, pools);
    record(
        t,
        &opposite_pairs,
        Market::Opposite,
        blocks,
        pools,
        births,
        y1,
        (key, plans),
        class_moves,
    );

    // Same-sex markets, merged per sex so each block pair has one cell per
    // kind and class.
    for sex in [Sex::Female, Sex::Male] {
        let mut pairs = Pairs::default();
        for (spec, x) in solved.by_ref().take(same_markets[sex as usize]) {
            debug_assert!(spec.market == Market::Same(sex));
            apply_market(t, spec, x, pools, &mut pairs);
        }
        defer_isolated(&mut pairs, Market::Same(sex), pools);
        record(
            t,
            &pairs,
            Market::Same(sex),
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
/// gives an entry's block and its `(class, partner block, couples)`. Each
/// list keeps the split's order (class cells sort their own).
fn by_block(
    split: &ClassSplit,
    side: impl Fn(&(u32, u32, u8, u64)) -> (u32, (u8, u32, u64)),
) -> ByBlock {
    // A stable sort by block keeps each block's entries in split order.
    let mut entries: Vec<(u32, (u8, u32, u64))> = split.iter().map(&side).collect();
    entries.sort_by_key(|e| e.0);
    let mut out: ByBlock = Vec::new();
    for (b, p) in entries {
        match out.last_mut() {
            Some(last) if last.0 == b => last.1.push(p),
            _ => out.push((b, vec![p])),
        }
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
    // A cell is (block, kind, area): the couple's area is the row's.
    let mut per_row: FxHashMap<(u32, CellKind, u16), u64> = FxHashMap::default();
    let mut per_col: FxHashMap<(u32, CellKind, u16), u64> = FxHashMap::default();
    for (&(rw, cw), &n) in pairs.iter() {
        let (kf, km) = market.kinds(rw, cw);
        *per_row.entry((rw.block, kf, rw.area)).or_default() += n;
        *per_col.entry((cw.block, km, rw.area)).or_default() += n;
    }
    pairs.retain(|&(rw, cw), &mut n| {
        let (kf, km) = market.kinds(rw, cw);
        let isolated = n == 1
            && per_row[&(rw.block, kf, rw.area)] == 1
            && per_col[&(cw.block, km, rw.area)] == 1;
        if isolated {
            // Both sides' wants are carried to next year.
            for (w, kind, sex) in [(rw, kf, rsex), (cw, km, csex)] {
                let pool = &mut pools[w.block as usize];
                pool.taken_mut(w.area)[kind as usize][sex as usize] -= 1;
                pool.carry_one(w.area, sex, w.divorced);
            }
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
    let areas_on = plans.params.places.by_area;
    // Cells are per kind and area: a couple lives in the row's (the woman's,
    // or the left partner's) area.
    type Grouped = BTreeMap<(CellKind, CellKind, u16), FxHashMap<(u32, u32), u64>>;
    let mut groups: Grouped = BTreeMap::new();
    // Each side's members by block and the area they live in (the column
    // side can join the row's area from another one).
    type Areas = BTreeMap<(CellKind, CellKind, u16), [BTreeMap<u32, Vec<(u16, u64)>>; 2]>;
    let mut member_areas: Areas = BTreeMap::new();
    for (&(rw, cw), &n) in pairs {
        let (kf, km) = market.kinds(rw, cw);
        *groups
            .entry((kf, km, rw.area))
            .or_default()
            .entry((rw.block, cw.block))
            .or_default() += n;
        let sides = member_areas.entry((kf, km, rw.area)).or_default();
        for (side, w) in [(0, rw), (1, cw)] {
            let list = sides[side].entry(w.block).or_default();
            match list.iter_mut().find(|e| e.0 == w.area) {
                Some(e) => e.1 += n,
                None => list.push((w.area, n)),
            }
        }
    }
    for ((kf, km, area), slices) in groups {
        let sides = member_areas
            .remove(&(kf, km, area))
            .expect("both sides' areas");
        let key = area_key(key, area, areas_on);
        let mut split = match market {
            Market::Opposite => split_classes(
                &slices,
                &SystematicShares::new(&plans.params.dissolution.class_pmf(t, kf.remarriage())),
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
        for (side_i, (sex, kind, grouped)) in [(rsex, kf, by_row), (csex, km, by_col)]
            .into_iter()
            .enumerate()
        {
            let areas_of = &sides[side_i];
            // Each block's cells, plans and sources touch only that block
            // and its pool, so the blocks are recorded on every core; their
            // births (sums) are merged after, so the result is the same.
            // Tasks borrow their partner lists, so the lists are freed here,
            // not across threads.
            let mut grouped = grouped;
            let mut tasks = Vec::with_capacity(grouped.len());
            // Each block's `&mut`, taken by splitting the slices at the
            // group's blocks (ascending), not by walking every block.
            let (mut rest_b, mut rest_p): (&mut [Block], &mut [Pool]) =
                (&mut blocks[..], &mut pools[..]);
            let mut base = 0usize;
            for (b, partners) in grouped.iter_mut() {
                let skip = *b as usize - base;
                let (block, tail_b) = std::mem::take(&mut rest_b)[skip..]
                    .split_first_mut()
                    .expect("every block recorded");
                let (pool, tail_p) = std::mem::take(&mut rest_p)[skip..]
                    .split_first_mut()
                    .expect("every pool recorded");
                (rest_b, rest_p) = (tail_b, tail_p);
                base = *b as usize + 1;
                let mut ar = areas_of[b].clone();
                ar.sort_unstable();
                tasks.push((*b, partners, block, pool, ar));
            }
            let side = Side {
                t,
                sex,
                kind,
                area,
                key,
                plans,
            };
            let born: Vec<(u32, Born)> = tasks
                .into_par_iter()
                .map(|(b, partners, block, pool, ar)| {
                    (b, side.record_block(b, partners, block, pool, &ar))
                })
                .collect();
            for (b, born) in born {
                let own = pools[b as usize].own;
                let here = born.here.iter().enumerate().map(|(o, &n)| (o as u8, area, n));
                for (o, a, n) in here.chain(born.moved) {
                    let year = t + o as i32;
                    if n > 0 && year <= y1 {
                        births.in_area(year, b, a, own).0 += n;
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
    /// The cells' area (area mode; 0 otherwise).
    area: u16,
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
        member_areas: &[(u16, u64)],
    ) -> Born {
        let Self {
            t,
            sex,
            kind,
            area,
            key,
            plans,
        } = *self;
        let n: u64 = partners.iter().map(|p| p.2).sum();
        let age = t - block.year;
        // Each area the members live in settles its own takings (the cell
        // is in the union's area, `area`).
        let mut members: Vec<(u32, u64)> = Vec::new();
        let from = if kind.second() {
            Members::Sources
        } else {
            Members::Cohorts
        };
        for &(a, na) in member_areas {
            if kind.second() {
                let skey =
                    key.with(TAG_SETTLE)
                        .with3(t as u64, b as u64, (kind as u64) << 1 | sex as u64);
                let skey = if a == area {
                    skey
                } else {
                    skey.with2(TAG_AREA, a as u64)
                };
                members.extend(pool.settle_div(
                    kind,
                    sex,
                    t,
                    skey,
                    &plans.params.repartnering,
                    a,
                    na,
                ));
            } else {
                members.extend(pool.settle(kind, sex, a, t, na));
            }
        }
        members.sort_unstable();
        debug_assert_eq!(members.iter().map(|c| c.1).sum::<u64>(), n);
        let cells = class_cells(
            t,
            kind,
            area,
            partners,
            &members,
            from,
            key.with(TAG_CONTINGENCY)
                .with3(t as u64, b as u64, (kind as u64) << 1 | sex as u64),
        );
        let mut born = Born {
            here: [0; 32],
            moved: Vec::new(),
        };
        let mut leaves: Vec<PlanLeaf> = Vec::new();
        for cell in cells {
            // Opposite-sex women's plans give births; same-sex unions have
            // none in R1.
            if sex == Sex::Female && !kind.same_sex() {
                let pk = area_key(
                    plan_key(plans.root, b, t, kind, cell.class),
                    area,
                    plans.params.places.by_area,
                );
                leaves.clear();
                plans.tables.at(t, block.heritage).plans_into(
                    cell.total,
                    kind.second(),
                    pk,
                    &mut leaves,
                );
                // Area mode: each leaf splits over the couples' moves
                // (stage 3); a child is born into the area of their
                // upbringing, where the family lives when they turn
                // `UPBRINGING_AGE` ([`upbringing_moved`]).
                let moves = plans
                    .moves
                    .map(|m| (m, m.offsets(t, age, cutoff(cell.class))));
                for (li, leaf) in leaves.iter().enumerate() {
                    let (offs, nb) =
                        leaf_births(leaf, age, cutoff(cell.class), &plans.params.fertility);
                    let mut add = |k: u8, dest: u16, n: u64| {
                        for &o in &offs[..nb] {
                            if upbringing_moved(o as i32, k, cutoff(cell.class)) {
                                born.moved.push((o, dest, n));
                            } else {
                                born.here[o as usize] += n;
                            }
                        }
                    };
                    match &moves {
                        Some((m, offsets)) => m.split(
                            offsets,
                            leaf.count,
                            t,
                            area,
                            move_key(pk, li),
                            &mut add,
                        ),
                        None => add(0, area, leaf.count),
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
                let d = &mut div[src as usize];
                d.parts.push((t, kind, cell.class, count));
                if plans.params.places.by_area {
                    d.part_areas.push(cell.area);
                }
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

/// A block side's plan births from one market group, by offset from the
/// union year: in the cells' area, and (area mode) in the areas couples
/// moved to, `(offset, area, births)`.
struct Born {
    here: [u64; 32],
    moved: Vec<(u8, u16, u64)>,
}

/// The age at which a child's area of upbringing is taken: their block's
/// area is where the family lives then (spec §2; stage 3).
pub const UPBRINGING_AGE: i32 = 18;

/// True if a child born `o` years after the union year grows up in the
/// destination of the couple's move `k` years after it (`k = 0`: no move):
/// the move comes by the child's `UPBRINGING_AGE` and the couple has not
/// separated by then (a separated couple that moved returns to where it
/// formed, spec §9). `cutoff` is the separation offset, if any.
pub fn upbringing_moved(o: i32, k: u8, cutoff: Option<i32>) -> bool {
    let at = o + UPBRINGING_AGE;
    k > 0 && at >= k as i32 && cutoff.is_none_or(|c| at < c)
}

/// The key of plan leaf `li`'s move split (stage 3), shared with the world.
pub fn move_key(plan_key: Key, li: usize) -> Key {
    plan_key.with2(TAG_MOVES, li as u64)
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
fn solve_market(
    t: i32,
    spec: &MarketSpec,
    blocks: &[Block],
    key: Key,
    u: &Unions,
    kernels: &Kernels,
) -> SparseCounts {
    let MarketSpec {
        mk,
        rows,
        cols,
        market,
    } = *spec;
    let (nr, nc) = (rows.len(), cols.len());
    if nr == 0 || nc == 0 {
        return SparseCounts::default();
    }
    let (sf, sm): (f64, f64) = (
        rows.iter().map(|p| p.1).sum(),
        cols.iter().map(|p| p.1).sum(),
    );
    if sf <= 0.0 || sm <= 0.0 {
        return SparseCounts::default();
    }
    let total = 2.0 * sf * sm / (sf + sm);
    let r: Vec<f64> = rows.iter().map(|p| p.1 * total / sf).collect();
    let c: Vec<f64> = cols.iter().map(|p| p.1 * total / sm).collect();
    // The kernel depends only on the birth-year gap and the statuses, so
    // the IPF runs over (birth year, status) classes and each seeker takes
    // its share of its class's margin (R1 plan, R1b-1).
    let class = |w: Who| (blocks[w.block as usize].year, w.divorced);
    let row_keys: Vec<(i32, bool)> = rows.iter().map(|p| class(p.0)).collect();
    let col_keys: Vec<(i32, bool)> = cols.iter().map(|p| class(p.0)).collect();
    // age_col - age_row = year_row - year_col
    let fit = GroupedIpf::new(
        &row_keys,
        &r,
        &col_keys,
        &c,
        |&(yf, df), &(ym, dm)| kernels.at(market, yf - ym, df, dm, u),
        IpfStop::DEFAULT,
    );

    // Keyed systematic rounding, row by row, in two levels: a row's points
    // fall first in a column class (its class's weights over the column
    // classes), then in a column of that class (the class's shares, the
    // same for every row). Every cell gets the floor or ceiling of its
    // expectation, exactly in expectation, and the row's total is within
    // one of its expectation. Cost: O(classes² + columns + couples · log)
    // instead of a draw per cell or a `classes × columns` table.
    let key = key.with2(t as u64, mk);
    let fit = &fit;
    let by = fit.columns_by_class();
    let gc = fit.cols.keys.len();
    let class_cum: Vec<f64> = (0..fit.rows.keys.len())
        .flat_map(|a| fit.class_weights_cumulative(a, &by))
        .collect();
    // Each row is a pure function of its seeker, so rows round in parallel.
    let rows_out: Vec<Vec<(u32, u64)>> = (0..nr)
        .into_par_iter()
        .map(|i| {
            let a = fit.rows.of[i];
            let mut out: Vec<(u32, u64)> = Vec::new();
            fit.round_row(
                i,
                &by,
                &class_cum[a * gc..(a + 1) * gc],
                key.with(rows[i].0.code()).unit(),
                |j, n| out.push((j as u32, n)),
            );
            out.sort_unstable();
            out
        })
        .collect();
    SparseCounts::from_rows(rows_out)
}

/// Apply a solved market's matrix `x`: capacity caps, then the takings in
/// the pools by cell kind, and the couples added to `pairs`. Rows' takings
/// count against the columns' caps when both sides share a pool (same-sex
/// markets), so a pool is never over-drawn.
fn apply_market(
    t: i32,
    spec: &MarketSpec,
    mut x: SparseCounts,
    pools: &mut [Pool],
    pairs: &mut Pairs,
) {
    if x.is_empty() {
        return;
    }
    let MarketSpec {
        rows, cols, market, ..
    } = *spec;
    let nc = cols.len();
    let (rsex, csex) = market.sexes();
    // Caps: never more unions than expected available survivors, and never
    // more than the seeker's available members. Rows first; their takings
    // count against the columns' caps when both sides share a pool.
    let mut row_take: FxHashMap<Who, u64> = FxHashMap::default();
    for (i, &(rw, _)) in rows.iter().enumerate() {
        let range = x.row(i);
        // Most seekers get no couple in a given market.
        if range.is_empty() {
            continue;
        }
        let row = &mut x.n[range];
        trim_largest_first(
            row,
            pools[rw.block as usize].cap(rsex, rw.divorced, t, rw.area),
        );
        *row_take.entry(rw).or_default() += row.iter().sum::<u64>();
    }
    // Entries by column, rows ascending.
    let (col_start, by_col) = x.column_index(nc);
    let mut col: Vec<u64> = Vec::new();
    for (j, &(cw, _)) in cols.iter().enumerate() {
        let entries = &by_col[col_start[j] as usize..col_start[j + 1] as usize];
        if entries.is_empty() {
            continue;
        }
        let shared = match market {
            Market::Same(_) => row_take.get(&cw).copied().unwrap_or(0),
            Market::Opposite => 0,
        };
        let cap = pools[cw.block as usize]
            .cap(csex, cw.divorced, t, cw.area)
            .saturating_sub(shared);
        col.clear();
        col.extend(entries.iter().map(|&(_, e)| x.n[e as usize]));
        trim_largest_first(&mut col, cap);
        for (&(i, e), v) in entries.iter().zip(&col) {
            let cut = x.n[e as usize] - v;
            if cut > 0 {
                x.n[e as usize] = *v;
                *row_take.get_mut(&rows[i as usize].0).unwrap() -= cut;
            }
        }
    }

    // Record the takings by cell kind and add the matrix to the pairs.
    for (i, &(rw, _)) in rows.iter().enumerate() {
        for e in x.row(i) {
            let n = x.n[e];
            if n > 0 {
                let cw = cols[x.col[e] as usize].0;
                let (kf, km) = market.kinds(rw, cw);
                pools[rw.block as usize].taken_mut(rw.area)[kf as usize][rsex as usize] += n;
                pools[cw.block as usize].taken_mut(cw.area)[km as usize][csex as usize] += n;
                *pairs.entry((rw, cw)).or_default() += n;
            }
        }
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
                let natives: u64 = b
                    .cohorts
                    .iter()
                    .filter(|c| c.arrival.is_none())
                    .map(|c| c.size)
                    .sum();
                assert_eq!(s, natives, "block {}", b.year);
            }
            assert_eq!(b.cohorts.iter().map(|c| c.size).sum::<u64>(), b.size);
            assert_eq!(b.cohorts.iter().map(|c| c.females).sum::<u64>(), b.females);
            // Natives (the stayers, then migration classes) come first.
            let n_native = b.cohorts.iter().take_while(|c| c.arrival.is_none()).count();
            assert!(n_native >= 1);
            assert!(b.cohorts[n_native..].iter().all(|c| c.arrival.is_some()));
        }
        // Cells split exactly over cohorts; nobody partners before the
        // year after arrival, and no cohort partners more members than it has.
        let immigrants: u64 = l
            .blocks
            .iter()
            .flat_map(|b| b.cohorts.iter().filter(|c| c.arrival.is_some()))
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
                    assert!(d.class > 0 && d.kind.feeds_divorced());
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
}
