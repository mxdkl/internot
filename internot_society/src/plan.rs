//! Fertility plans, apportioned exactly.
//!
//! A union sub-cell (a block's women who start a union in one year and whose
//! couples share a dissolution class, R1c) is split into plan leaves by
//! nested keyed systematic apportionment: parity, then first birth offset,
//! then spacing pattern. Births stop before the couple's separation year and
//! at [`MAX_BIRTH_AGE`]. Each leaf is a pure function of `(sub-cell size,
//! union year, key)`, so the counts are never stored, and births per year are
//! exact integers on both the mother's side and the child block's side.
//! Systematic rounding, unlike largest remainder, gives small sub-cells the
//! rarer plans in proportion instead of always the modal one.
//!
//! Never-partnered and pre-union births come from a separate *non-union*
//! partition of all a block's women (0, 1 or 2 births at planned ages).

use procedural_core::key::Key;
use procedural_core::life::first_event_pmf;
use procedural_core::partition::{
    apportion_largest_remainder, apportion_systematic, SystematicShares,
};

use crate::params::{Fertility, Heritage, Heritages, Params, Sex, MAX_BIRTH_AGE, MIN_BIRTH_AGE};

/// [`apportion_systematic`] into a new vector.
fn split(n: u64, weights: &[f64], key: Key) -> Vec<u64> {
    let mut out = vec![0; weights.len()];
    apportion_systematic(n, weights, key, &mut out);
    out
}

/// [`apportion_systematic`] into an array (no allocation).
#[cfg(test)]
fn split_n<const N: usize>(n: u64, weights: &[f64; N], key: Key) -> [u64; N] {
    let mut out = [0; N];
    apportion_systematic(n, weights, key, &mut out);
    out
}

/// Maximum planned births in a union.
pub const MAX_PARITY: usize = 8;

/// One leaf of a union cell's plan partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanLeaf {
    /// Planned parity before truncation.
    pub parity: u8,
    /// Index into the first-birth-offset pmf (offset = index + 1 years).
    pub first: u8,
    /// Index into the pack's spacing patterns ([`Fertility::spacing`]).
    pub spacing: u8,
    /// Women in this leaf.
    pub count: u64,
}

/// Birth offsets (years after the union year) actually realised by a leaf,
/// for a mother who is `mother_age` in the union year. Births stop at
/// [`MAX_BIRTH_AGE`] and, if the couple separates `cutoff` years after the
/// union year, before that year: the union is intact at every birth.
pub fn leaf_births(
    leaf: &PlanLeaf,
    mother_age: i32,
    cutoff: Option<i32>,
    fert: &Fertility,
) -> ([u8; MAX_PARITY], usize) {
    let mut out = [0u8; MAX_PARITY];
    let mut n = 0;
    let mut offset = leaf.first as i32 + 1;
    for k in 0..leaf.parity as usize {
        if k > 0 {
            offset += fert.spacing_gap(leaf.spacing as usize, k as u32) as i32;
        }
        if mother_age + offset > MAX_BIRTH_AGE || cutoff.is_some_and(|c| offset >= c) {
            break;
        }
        out[n] = offset as u8;
        n += 1;
    }
    (out, n)
}

/// The plan partition of a union sub-cell of `n` women who start unions in
/// `union_year`, keyed per sub-cell. Nonzero leaves only, in canonical order.
/// Planned births past [`MAX_BIRTH_AGE`] or after the separation are dropped
/// per mother in [`leaf_births`], so older brides and early separations
/// realise fewer births from the same plans.
pub fn union_plans(n: u64, union_year: i32, key: Key, fert: &Fertility) -> Vec<PlanLeaf> {
    let mut out = Vec::new();
    PlanShares::new(union_year, fert, 1.0).plans_into(n, false, key, &mut out);
    out
}

/// [`union_plans`] for women in a re-partnering union (R1c), whose parity
/// schedule is [`Fertility::second_union_parity_pmf`].
pub fn second_union_plans(n: u64, union_year: i32, key: Key, fert: &Fertility) -> Vec<PlanLeaf> {
    let mut out = Vec::new();
    PlanShares::new(union_year, fert, 1.0).plans_into(n, true, key, &mut out);
    out
}

/// The shares of one union year's plan partitions, computed once for all
/// of the year's sub-cells: parity (first and second unions), first-birth
/// offset and spacing ([`SystematicShares`]).
#[derive(Clone, Debug)]
pub struct PlanShares {
    parity: [SystematicShares; 2],
    first: SystematicShares,
    spacing: SystematicShares,
}

impl PlanShares {
    /// The shares of `union_year`'s schedules for a group whose fertility is
    /// `factor` times the base.
    pub fn new(union_year: i32, fert: &Fertility, factor: f64) -> Self {
        Self {
            parity: [
                SystematicShares::new(&fert.union_parity_pmf(union_year, factor)),
                SystematicShares::new(&fert.second_union_parity_pmf(union_year, factor)),
            ],
            first: SystematicShares::new(&fert.first_birth_offset_pmf(union_year)),
            spacing: SystematicShares::new(&fert.spacing_weights()),
        }
    }

    /// Appends the plan partition of `n` women in a first or (`second`)
    /// second union to `out`: nested keyed systematic splits by parity, then
    /// first-birth offset, then spacing pattern. Childless plans don't need
    /// timing branches.
    pub fn plans_into(&self, n: u64, second: bool, key: Key, out: &mut Vec<PlanLeaf>) {
        self.parity[second as usize].for_each_part(n, key.with(0), |p, np| {
            if p == 0 {
                out.push(PlanLeaf {
                    parity: 0,
                    first: 0,
                    spacing: 0,
                    count: np,
                });
                return;
            }
            self.first
                .for_each_part(np, key.with2(1, p as u64), |f, nf| {
                    self.spacing
                        .for_each_part(nf, key.with3(2, p as u64, f as u64), |s, ns| {
                            out.push(PlanLeaf {
                                parity: p as u8,
                                first: f as u8,
                                spacing: s as u8,
                                count: ns,
                            });
                        });
                });
        });
    }
}

/// [`PlanShares`] for every union year in a range and every heritage
/// group (whose fertility factors differ).
#[derive(Clone, Debug)]
pub struct PlanTables {
    first_year: i32,
    groups: usize,
    /// Row `(year − first_year) · groups + group`.
    shares: Vec<PlanShares>,
}

impl PlanTables {
    /// Shares for union years `first_year..=last_year`.
    pub fn new(first_year: i32, last_year: i32, fert: &Fertility, her: &Heritages) -> Self {
        let groups = her.groups.len();
        Self {
            first_year,
            groups,
            shares: (first_year..=last_year)
                .flat_map(|y| (0..groups).map(move |h| (y, Heritage(h as u8))))
                .map(|(y, h)| PlanShares::new(y, fert, her.fertility_factor(h, y)))
                .collect(),
        }
    }

    /// The shares of `union_year` for group `h`. Panics outside the range.
    pub fn at(&self, union_year: i32, h: Heritage) -> &PlanShares {
        let i = union_year - self.first_year;
        assert!(
            i >= 0 && (i as usize) < self.shares.len() / self.groups,
            "union year {union_year} outside the plan tables"
        );
        &self.shares[i as usize * self.groups + h.index()]
    }
}

/// One leaf of an arriving couple's plan (R1 plan, R1b-2 step B): the
/// union began `d` years before arrival; the other fields are a
/// [`PlanLeaf`]'s, counted from the union year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArrivalLeaf {
    /// Years of union before arrival.
    pub d: u8,
    /// The plan, from the union year `arrival − d`.
    pub plan: PlanLeaf,
}

/// Births of an arriving couple's leaf, for a woman aged `age` in the
/// arrival year, whose couple separates `class` years after arrival (0:
/// never): `(in_world, kids)`. Bit `o` of `in_world` is a birth `o` years
/// after arrival (`o >= 1`: births in later years are in-world); bit `c` of
/// `kids` is a child born abroad (in the arrival year or before) who arrives
/// aged `c`, at most the pack's `max_child_age`. Older children stay abroad
/// and are not people.
pub fn arrival_births(leaf: &ArrivalLeaf, age: i32, class: u8, p: &Params) -> (u32, u32) {
    let d = leaf.d as i32;
    let cutoff = (class > 0).then_some(d + class as i32);
    let (offs, n) = leaf_births(&leaf.plan, age - d, cutoff, &p.fertility);
    let (mut in_world, mut kids) = (0u32, 0u32);
    for &o in &offs[..n] {
        let o = o as i32;
        if o > d {
            in_world |= 1 << (o - d);
        } else if d - o <= p.immigration.max_child_age {
            kids |= 1 << (d - o);
        }
    }
    (in_world, kids)
}

/// The plan partition of `n` women who arrive with their partners in
/// `arrival_year`, aged `age`, keyed per sub-cell. Nested systematic
/// apportionment, outermost first:
/// - `d`, the union's years before arrival, from the first-union schedule
///   (the union started at `age - d >=` the pack's `min_couple_age`);
/// - then as [`union_plans`], using the union year's schedules. Separation
///   comes from the couple's class, counted from arrival, so it always
///   follows arrival: the couple arrives together.
///
/// `dens` is [`union_age_density`] of the arrival year, covering `age`;
/// `tables` cover the union years.
#[allow(clippy::too_many_arguments)]
pub fn arrival_plans(
    n: u64,
    arrival_year: i32,
    age: i32,
    key: Key,
    dens: &[f64],
    tables: &PlanTables,
    h: Heritage,
    min_couple_age: i32,
) -> Vec<ArrivalLeaf> {
    let mut leaves = Vec::new();
    if n == 0 || age < min_couple_age {
        return leaves;
    }
    // Weight of d = age - union age, for d = 0, 1, ...
    let d_weights: Vec<f64> = (0..=age - min_couple_age)
        .map(|d| dens[(age - d) as usize])
        .collect();
    let mut plans = Vec::new();
    for (d, &nd) in split(n, &d_weights, key.with(3)).iter().enumerate() {
        if nd == 0 {
            continue;
        }
        let union_year = arrival_year - d as i32;
        plans.clear();
        tables
            .at(union_year, h)
            .plans_into(nd, false, key.with2(4, d as u64), &mut plans);
        leaves.extend(plans.iter().map(|&plan| ArrivalLeaf { d: d as u8, plan }));
    }
    leaves
}

/// Density of a first union at each age `0..=max_age` under `year`'s
/// schedule, for women (the weights of an arriving couple's years of union
/// before arrival, [`arrival_plans`]). Computed once per arrival year: it
/// costs a hazard evaluation per age.
pub fn union_age_density(year: i32, max_age: i32, p: &Params) -> Vec<f64> {
    let mut d = first_event_pmf(max_age, |a| {
        p.unions.first_union_hazard(Sex::Female, a as u32, year)
    });
    let young = (p.immigration.min_couple_age.max(0) as usize).min(d.len());
    d[..young].fill(0.0);
    d
}

/// One leaf of a block's non-union birth partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonUnionLeaf {
    /// Ages at the births (only the first `births` are used).
    pub ages: [u8; 2],
    /// 0, 1 or 2 births.
    pub births: u8,
    /// Women in this leaf.
    pub count: u64,
}

/// The non-union partition of a block of `females` women born in
/// `block_year`. Nonzero leaves only, in canonical order: no births first.
/// `factor` is the group's fertility factor in the year of the women's 25th
/// birthday.
pub fn nonunion_plans(
    females: u64,
    block_year: i32,
    fert: &Fertility,
    factor: f64,
) -> Vec<NonUnionLeaf> {
    let mut leaves = Vec::new();
    let counts =
        apportion_largest_remainder(females, &fert.nonunion_count_pmf(block_year + 25, factor));
    if counts[0] > 0 {
        leaves.push(NonUnionLeaf {
            ages: [0, 0],
            births: 0,
            count: counts[0],
        });
    }
    let ages: Vec<i32> = (MIN_BIRTH_AGE..MAX_BIRTH_AGE).collect();
    let weights: Vec<f64> = ages.iter().map(|&a| fert.nonunion_age_weight(a)).collect();
    let gaps = &fert.nonunion.second_gaps;
    let gap_weights = vec![1.0; gaps.len()];
    for (i, &n) in apportion_largest_remainder(counts[1], &weights)
        .iter()
        .enumerate()
    {
        if n > 0 {
            leaves.push(NonUnionLeaf {
                ages: [ages[i] as u8, 0],
                births: 1,
                count: n,
            });
        }
    }
    for (i, &n) in apportion_largest_remainder(counts[2], &weights)
        .iter()
        .enumerate()
    {
        if n == 0 {
            continue;
        }
        for (g, &ng) in apportion_largest_remainder(n, &gap_weights)
            .iter()
            .enumerate()
        {
            let second = ages[i] + gaps[g] as i32;
            if ng > 0 && second < MAX_BIRTH_AGE {
                leaves.push(NonUnionLeaf {
                    ages: [ages[i] as u8, second as u8],
                    births: 2,
                    count: ng,
                });
            } else if ng > 0 {
                leaves.push(NonUnionLeaf {
                    ages: [ages[i] as u8, 0],
                    births: 1,
                    count: ng,
                });
            }
        }
    }
    leaves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us() -> Params {
        Params::prototype()
    }

    /// The plan partition as first written, with a full split at every
    /// level: [`PlanShares::plans_into`] must match it exactly.
    fn reference_plans(
        n: u64,
        union_year: i32,
        parity_pmf: &[f64; 9],
        key: Key,
        fert: &Fertility,
    ) -> Vec<PlanLeaf> {
        let mut leaves = Vec::new();
        if n == 0 {
            return leaves;
        }
        let parity = split_n(n, parity_pmf, key.with(0));
        let first_pmf = fert.first_birth_offset_pmf(union_year);
        let spacing: [f64; 3] = fert.spacing_weights().try_into().unwrap();
        for (p, &np) in parity.iter().enumerate() {
            if np == 0 {
                continue;
            }
            let firsts = if p == 0 {
                [np, 0, 0, 0]
            } else {
                split_n(np, &first_pmf, key.with2(1, p as u64))
            };
            for (f, &nf) in firsts.iter().enumerate() {
                if nf == 0 {
                    continue;
                }
                let spacing_counts = if p == 0 {
                    [nf, 0, 0]
                } else {
                    split_n(nf, &spacing, key.with3(2, p as u64, f as u64))
                };
                for (s, &ns) in spacing_counts.iter().enumerate() {
                    if ns > 0 {
                        leaves.push(PlanLeaf {
                            parity: p as u8,
                            first: f as u8,
                            spacing: s as u8,
                            count: ns,
                        });
                    }
                }
            }
        }
        leaves
    }

    #[test]
    fn precomputed_plans_match_the_reference() {
        let p = us();
        let f = &p.fertility;
        let tables = PlanTables::new(1830, 2100, f, &p.heritage);
        for year in (1830..=2100).step_by(7) {
            for n in (0u64..40).chain([97, 1000, 54_321]) {
                for seed in 0..6 {
                    let key = Key::from_seed(seed * 1000 + n);
                    assert_eq!(
                        union_plans(n, year, key, f),
                        reference_plans(n, year, &f.union_parity_pmf(year, 1.0), key, f)
                    );
                    // A group with its own fertility factor: the tables tilt.
                    let h = Heritage(1);
                    let factor = p.heritage.fertility_factor(h, year);
                    let mut second = Vec::new();
                    tables.at(year, h).plans_into(n, true, key, &mut second);
                    assert_eq!(
                        second,
                        reference_plans(n, year, &f.second_union_parity_pmf(year, factor), key, f)
                    );
                }
            }
        }
    }

    #[test]
    fn plans_cover_the_cell_exactly() {
        let p = us();
        for &(n, year) in &[(1u64, 1900), (17, 1957), (5000, 2024), (300, 1840)] {
            let leaves = union_plans(n, year, Key::from_seed(n), &p.fertility);
            assert_eq!(leaves.iter().map(|l| l.count).sum::<u64>(), n);
        }
    }

    #[test]
    fn small_sub_cells_draw_plans_in_proportion() {
        // One-woman sub-cells: over many keys, her parity follows the pmf,
        // not the modal parity every time.
        let p = us();
        let pmf = p.fertility.union_parity_pmf(1950, 1.0);
        let mut hits = [0u64; 9];
        let trials = 20_000u64;
        for seed in 0..trials {
            let leaves = union_plans(1, 1950, Key::from_seed(seed), &p.fertility);
            hits[leaves[0].parity as usize] += 1;
        }
        let total: f64 = pmf.iter().sum();
        for (h, p) in hits.iter().zip(pmf) {
            assert!((*h as f64 / trials as f64 - p / total).abs() < 0.012);
        }
    }

    #[test]
    fn births_respect_age_and_separation() {
        let leaf = PlanLeaf {
            parity: 8,
            first: 0,
            spacing: 0,
            count: 1,
        };
        let p = us();
        let (offs, n) = leaf_births(&leaf, 30, Some(4), &p.fertility);
        // Separation 4 years after the union year: births at 1, 2 (1+1); 4 is excluded.
        assert_eq!(&offs[..n], &[1, 2]);
        let leaf = PlanLeaf {
            parity: 5,
            first: 1,
            spacing: 1,
            count: 1,
        };
        let (offs, n) = leaf_births(&leaf, 40, None, &p.fertility);
        // 2, 4, 7 → ages 42, 44, 47: the last exceeds 45.
        assert_eq!(&offs[..n], &[2, 4]);
    }

    #[test]
    fn arrival_plans_cover_the_couples_and_split_births() {
        let p = us();
        let mca = p.immigration.min_couple_age;
        let tables = PlanTables::new(1820, 2000, &p.fertility, &p.heritage);
        for &(n, year, age) in &[
            (1u64, 1900, 19),
            (40, 1905, 33),
            (500, 1990, 41),
            (7, 2000, 70),
        ] {
            let dens = union_age_density(year, age, &p);
            let leaves = arrival_plans(
                n,
                year,
                age,
                Key::from_seed(n),
                &dens,
                &tables,
                Heritage(0),
                mca,
            );
            assert_eq!(leaves.iter().map(|l| l.plan.count).sum::<u64>(), n);
            for leaf in &leaves {
                let d = leaf.d as i32;
                assert!(age - d >= mca);
                // Separating 2 years after arrival leaves no later births.
                let (late, _) = arrival_births(leaf, age, 2, &p);
                assert_eq!(late >> 2, 0, "a birth after the separation");
                let (in_world, kids) = arrival_births(leaf, age, 0, &p);
                assert_eq!(in_world & 1, 0);
                assert!(kids < 1 << (p.immigration.max_child_age + 1));
                // Every in-world birth is by the mother's 45th birthday.
                if in_world != 0 {
                    assert!(age + 31 - (in_world.leading_zeros() as i32) <= MAX_BIRTH_AGE);
                }
            }
        }
        let dens = union_age_density(1950, 18, &p);
        assert!(arrival_plans(
            5,
            1950,
            18,
            Key::from_seed(1),
            &dens,
            &tables,
            Heritage(0),
            mca
        )
        .is_empty());
    }

    #[test]
    fn nonunion_plans_cover_all_women() {
        let p = us();
        for &(f, y) in &[(0u64, 1900), (1, 1950), (40_000, 2000)] {
            let leaves = nonunion_plans(f, y, &p.fertility, 1.0);
            assert_eq!(leaves.iter().map(|l| l.count).sum::<u64>(), f);
            assert!(leaves
                .iter()
                .all(|l| l.births == 0 || l.ages[0] >= MIN_BIRTH_AGE as u8));
        }
    }
}
