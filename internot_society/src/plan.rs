//! Fertility and dissolution plans, apportioned exactly.
//!
//! A union cell (a block's women who start a union in one year) is split into
//! plan leaves by nested largest-remainder apportionment: parity, then first
//! birth offset, then spacing pattern, then dissolution band. Each leaf is a
//! pure function of `(cell size, union year, mother's age)`, so the counts
//! are never stored, and births per year are exact integers on both the
//! mother's side and the child block's side.
//!
//! Never-partnered and pre-union births come from a separate *non-union*
//! partition of all a block's women (0, 1 or 2 births at planned ages).

use crate::params::{
    dissolution_pmf, first_birth_offset_pmf, first_union_hazard, nonunion_age_weight,
    nonunion_count_pmf, union_parity_pmf, Sex, DISSOLUTION_BANDS, MAX_BIRTH_AGE,
    MAX_CHILD_ARRIVAL_AGE, MIN_BIRTH_AGE, MIN_COUPLE_ARRIVAL_AGE, SPACING, SPACING_PMF,
};

/// Split `n` into integer parts proportional to `weights` (largest
/// remainder; ties go to the lower index). The parts always sum to `n`
/// when any weight is positive.
pub fn apportion(n: u64, weights: &[f64]) -> Vec<u64> {
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    let mut out = vec![0u64; weights.len()];
    if n == 0 || total <= 0.0 {
        return out;
    }
    let mut assigned = 0u64;
    let mut rema: Vec<(f64, usize)> = Vec::with_capacity(weights.len());
    for (i, &w) in weights.iter().enumerate() {
        if w <= 0.0 {
            continue;
        }
        let exact = n as f64 * w / total;
        let base = exact.floor() as u64;
        out[i] = base;
        assigned += base;
        rema.push((exact - base as f64, i));
    }
    rema.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, i) in rema.iter().take((n - assigned) as usize) {
        out[i] += 1;
    }
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
    /// Index into [`SPACING`].
    pub spacing: u8,
    /// Index into [`DISSOLUTION_BANDS`].
    pub dissolution: u8,
    /// Women in this leaf.
    pub count: u64,
}

/// Birth offsets (years after the union year) actually realised by a leaf,
/// for a mother who is `mother_age` in the union year. Births stop at
/// [`MAX_BIRTH_AGE`] and before any planned dissolution.
pub fn leaf_births(leaf: &PlanLeaf, mother_age: i32) -> ([u8; MAX_PARITY], usize) {
    let mut out = [0u8; MAX_PARITY];
    let mut n = 0;
    let cutoff = DISSOLUTION_BANDS[leaf.dissolution as usize].map(|(lo, _)| lo as i32);
    let mut offset = leaf.first as i32 + 1;
    for k in 0..leaf.parity as usize {
        if k > 0 {
            offset += SPACING[leaf.spacing as usize][(k - 1) % 2] as i32;
        }
        if mother_age + offset > MAX_BIRTH_AGE || cutoff.is_some_and(|c| offset >= c) {
            break;
        }
        out[n] = offset as u8;
        n += 1;
    }
    (out, n)
}

/// The plan partition of a union cell of `n` women who start unions in
/// `union_year`. Nonzero leaves only, in canonical order. Planned births
/// past [`MAX_BIRTH_AGE`] are dropped per mother in [`leaf_births`], so older
/// brides realise fewer births from the same plans.
pub fn union_plans(n: u64, union_year: i32) -> Vec<PlanLeaf> {
    plans_with(n, union_year, &dissolution_pmf(union_year))
}

/// [`union_plans`] with a given dissolution pmf.
fn plans_with(n: u64, union_year: i32, diss_pmf: &[f64; 6]) -> Vec<PlanLeaf> {
    let mut leaves = Vec::new();
    let parity = apportion(n, &union_parity_pmf(union_year));
    let first_pmf = first_birth_offset_pmf(union_year);
    for (p, &np) in parity.iter().enumerate() {
        if np == 0 {
            continue;
        }
        // Childless plans don't need timing branches.
        let (firsts, spacings) = if p == 0 {
            (vec![np, 0, 0, 0], None)
        } else {
            (apportion(np, &first_pmf), Some(()))
        };
        for (f, &nf) in firsts.iter().enumerate() {
            if nf == 0 {
                continue;
            }
            let spacing_counts = if spacings.is_some() {
                apportion(nf, &SPACING_PMF)
            } else {
                vec![nf, 0, 0]
            };
            for (s, &ns) in spacing_counts.iter().enumerate() {
                if ns == 0 {
                    continue;
                }
                for (d, &nd) in apportion(ns, diss_pmf).iter().enumerate() {
                    if nd > 0 {
                        leaves.push(PlanLeaf {
                            parity: p as u8,
                            first: f as u8,
                            spacing: s as u8,
                            dissolution: d as u8,
                            count: nd,
                        });
                    }
                }
            }
        }
    }
    leaves
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
/// arrival year: `(in_world, kids)`. Bit `o` of `in_world` is a birth `o`
/// years after arrival (`o >= 1`: births in later years are in-world); bit
/// `c` of `kids` is a child born abroad (in the arrival year or before) who
/// arrives aged `c`, at most [`MAX_CHILD_ARRIVAL_AGE`]. Older children stay
/// abroad and are not people.
pub fn arrival_births(leaf: &ArrivalLeaf, age: i32) -> (u32, u32) {
    let d = leaf.d as i32;
    let (offs, n) = leaf_births(&leaf.plan, age - d);
    let (mut in_world, mut kids) = (0u32, 0u32);
    for &o in &offs[..n] {
        let o = o as i32;
        if o > d {
            in_world |= 1 << (o - d);
        } else if d - o <= MAX_CHILD_ARRIVAL_AGE {
            kids |= 1 << (d - o);
        }
    }
    (in_world, kids)
}

/// The plan partition of `n` women who arrive with their partners in
/// `arrival_year`, aged `age`. Nested apportionment, outermost first:
/// - `d`, the union's years before arrival, from the first-union schedule
///   (the union started at `age - d >=` [`MIN_COUPLE_ARRIVAL_AGE`]);
/// - then as [`union_plans`], using the union year's schedules, except that
///   only dissolution bands starting after arrival are possible: the couple
///   arrives together.
pub fn arrival_plans(n: u64, arrival_year: i32, age: i32) -> Vec<ArrivalLeaf> {
    let mut leaves = Vec::new();
    if n == 0 || age < MIN_COUPLE_ARRIVAL_AGE {
        return leaves;
    }
    // Density of a first union at each age, among women partnered by `age`
    // (period schedule of the arrival year).
    let mut never = 1.0;
    let mut dens = Vec::with_capacity((age + 1) as usize);
    for a in 0..=age {
        let h = first_union_hazard(Sex::Female, a as u32, arrival_year);
        dens.push(if a >= MIN_COUPLE_ARRIVAL_AGE {
            never * h
        } else {
            0.0
        });
        never *= 1.0 - h;
    }
    // Weight of d = age - union age, for d = 0, 1, ...
    let d_weights: Vec<f64> = (0..=age - MIN_COUPLE_ARRIVAL_AGE)
        .map(|d| dens[(age - d) as usize])
        .collect();
    for (d, &nd) in apportion(n, &d_weights).iter().enumerate() {
        if nd == 0 {
            continue;
        }
        let union_year = arrival_year - d as i32;
        let mut diss = dissolution_pmf(union_year);
        for (band, w) in DISSOLUTION_BANDS.iter().zip(diss.iter_mut()) {
            if band.is_some_and(|(lo, _)| lo as usize <= d) {
                *w = 0.0;
            }
        }
        for plan in plans_with(nd, union_year, &diss) {
            leaves.push(ArrivalLeaf { d: d as u8, plan });
        }
    }
    leaves
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

/// Gaps (years) between the two births of a two-birth non-union plan.
const NU_GAPS: [u8; 3] = [2, 3, 5];

/// The non-union partition of a block of `females` women born in
/// `block_year`. Nonzero leaves only, in canonical order: no births first.
pub fn nonunion_plans(females: u64, block_year: i32) -> Vec<NonUnionLeaf> {
    let mut leaves = Vec::new();
    let counts = apportion(females, &nonunion_count_pmf(block_year + 25));
    if counts[0] > 0 {
        leaves.push(NonUnionLeaf {
            ages: [0, 0],
            births: 0,
            count: counts[0],
        });
    }
    let ages: Vec<i32> = (MIN_BIRTH_AGE..MAX_BIRTH_AGE).collect();
    let weights: Vec<f64> = ages.iter().map(|&a| nonunion_age_weight(a)).collect();
    for (i, &n) in apportion(counts[1], &weights).iter().enumerate() {
        if n > 0 {
            leaves.push(NonUnionLeaf {
                ages: [ages[i] as u8, 0],
                births: 1,
                count: n,
            });
        }
    }
    for (i, &n) in apportion(counts[2], &weights).iter().enumerate() {
        if n == 0 {
            continue;
        }
        for (g, &ng) in apportion(n, &[1.0, 1.0, 1.0]).iter().enumerate() {
            let second = ages[i] + NU_GAPS[g] as i32;
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

    #[test]
    fn apportion_is_exact_and_proportional() {
        assert_eq!(apportion(10, &[1.0, 1.0, 1.0]), vec![4, 3, 3]);
        assert_eq!(apportion(0, &[1.0, 2.0]), vec![0, 0]);
        assert_eq!(apportion(7, &[0.0, 1.0]), vec![0, 7]);
        for n in [1u64, 13, 999, 123_457] {
            let v = apportion(n, &[0.2, 0.5, 0.3, 0.0, 1e-9]);
            assert_eq!(v.iter().sum::<u64>(), n);
        }
    }

    #[test]
    fn plans_cover_the_cell_exactly() {
        for &(n, year) in &[(1u64, 1900), (17, 1957), (5000, 2024), (300, 1840)] {
            let leaves = union_plans(n, year);
            assert_eq!(leaves.iter().map(|l| l.count).sum::<u64>(), n);
        }
    }

    #[test]
    fn births_respect_age_and_dissolution() {
        let leaf = PlanLeaf {
            parity: 8,
            first: 0,
            spacing: 0,
            dissolution: 2,
            count: 1,
        };
        let (offs, n) = leaf_births(&leaf, 30);
        // Dissolution band 2 starts at 4 years: births at 1, 2 (1+1), 4 is excluded.
        assert_eq!(&offs[..n], &[1, 2]);
        let leaf = PlanLeaf {
            parity: 5,
            first: 1,
            spacing: 1,
            dissolution: 0,
            count: 1,
        };
        let (offs, n) = leaf_births(&leaf, 40);
        // 2, 4, 7 → ages 42, 44, 47: the last exceeds 45.
        assert_eq!(&offs[..n], &[2, 4]);
    }

    #[test]
    fn arrival_plans_cover_the_couples_and_split_births() {
        for &(n, year, age) in &[
            (1u64, 1900, 19),
            (40, 1905, 33),
            (500, 1990, 41),
            (7, 2000, 70),
        ] {
            let leaves = arrival_plans(n, year, age);
            assert_eq!(leaves.iter().map(|l| l.plan.count).sum::<u64>(), n);
            for leaf in &leaves {
                let d = leaf.d as i32;
                assert!(age - d >= MIN_COUPLE_ARRIVAL_AGE);
                // No separation before arrival.
                if let Some((lo, _)) = DISSOLUTION_BANDS[leaf.plan.dissolution as usize] {
                    assert!(lo as i32 > d, "separates before arriving");
                }
                let (in_world, kids) = arrival_births(leaf, age);
                assert_eq!(in_world & 1, 0);
                assert!(kids < 1 << (MAX_CHILD_ARRIVAL_AGE + 1));
                // Every in-world birth is by the mother's 45th birthday.
                if in_world != 0 {
                    assert!(age + 31 - (in_world.leading_zeros() as i32) <= MAX_BIRTH_AGE);
                }
            }
        }
        assert!(arrival_plans(5, 1950, 18).is_empty());
    }

    #[test]
    fn nonunion_plans_cover_all_women() {
        for &(f, y) in &[(0u64, 1900), (1, 1950), (40_000, 2000)] {
            let leaves = nonunion_plans(f, y);
            assert_eq!(leaves.iter().map(|l| l.count).sum::<u64>(), f);
            assert!(leaves
                .iter()
                .all(|l| l.births == 0 || l.ages[0] >= MIN_BIRTH_AGE as u8));
        }
    }
}
