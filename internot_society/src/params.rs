//! Demographic parameters: era-dependent schedules for mortality, unions,
//! fertility and dissolution.
//!
//! These are prototype schedules (spec §18, R1), shaped on US history. They
//! are calibrated only to sanity bands: e₀, TFR, age at first union, parity.
//! The production ledger replaces them with tabulated series (NCHS life
//! tables, ASFR, CPS). Every schedule is a pure function of its arguments,
//! and all math goes through `procedural_core::dmath`, so results are
//! bit-identical across machines.

use procedural_core::dmath::{exp, ln};

/// Sex. `Female` first: women's rows index union tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sex {
    Female,
    Male,
}

/// World-level parameters.
#[derive(Clone, Debug)]
pub struct Params {
    /// First simulated year. People alive then are founders.
    pub y0: i32,
    /// Last simulated year (inclusive). Blocks exist for births up to it.
    pub y1: i32,
    /// Oldest founder age at `y0`.
    pub founder_max_age: i32,
    /// Expected births in the year before `y0`. Sets the world's scale.
    pub founder_births: f64,
    /// Growth rate of the stable population founders are drawn from.
    pub founder_growth: f64,
    /// Share of births that are male.
    pub male_share_at_birth: f64,
    /// Lineage regions (at least one). A child belongs to its mother's
    /// region (R1 plan D-R1.1).
    pub regions: Vec<Region>,
    /// Multiplier on [`same_sex_share`]: 1 for realistic worlds; test worlds
    /// raise it so exhaustive checks cover same-sex couples.
    pub same_sex_boost: f64,
}

/// One lineage region.
#[derive(Clone, Debug)]
pub struct Region {
    /// Short name, for reports.
    pub name: &'static str,
    /// Relative share of the founder population.
    pub founder_weight: f64,
    /// Relative share of immigrants arriving from abroad.
    pub immigrant_weight: f64,
}

impl Params {
    /// Prototype scale: two lineage regions, ~tens of thousands of births
    /// a year, 1840 to 2100.
    pub fn prototype() -> Self {
        Self {
            y0: 1840,
            y1: 2100,
            founder_max_age: 90,
            founder_births: 20_000.0,
            founder_growth: 0.025,
            male_share_at_birth: 1.05 / 2.05,
            regions: vec![
                Region {
                    name: "east",
                    founder_weight: 0.6,
                    immigrant_weight: 0.7,
                },
                Region {
                    name: "west",
                    founder_weight: 0.4,
                    immigrant_weight: 0.3,
                },
            ],
            same_sex_boost: 1.0,
        }
    }

    /// A small world for exhaustive tests.
    pub fn tiny() -> Self {
        Self {
            y0: 1900,
            y1: 1990,
            founder_max_age: 80,
            founder_births: 300.0,
            founder_growth: 0.01,
            male_share_at_birth: 1.05 / 2.05,
            regions: vec![
                Region {
                    name: "east",
                    founder_weight: 0.5,
                    immigrant_weight: 0.6,
                },
                Region {
                    name: "west",
                    founder_weight: 0.5,
                    immigrant_weight: 0.4,
                },
            ],
            same_sex_boost: 10.0,
        }
    }

    /// Number of lineage regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

/// Piecewise-linear interpolation over `(year, value)` anchors (clamped).
pub(crate) fn interp(anchors: &[(i32, f64)], year: i32) -> f64 {
    let y = year as f64;
    if y <= anchors[0].0 as f64 {
        return anchors[0].1;
    }
    for w in anchors.windows(2) {
        let (a, b) = (w[0], w[1]);
        if y <= b.0 as f64 {
            let f = (y - a.0 as f64) / (b.0 - a.0) as f64;
            return a.1 + f * (b.1 - a.1);
        }
    }
    anchors[anchors.len() - 1].1
}

// --- mortality ---------------------------------------------------------------

/// Siler hazard multipliers `(infant, background_female, background_male, old)`
/// at anchor years, relative to a 2024 US base. Calibrated (see the R1 plan)
/// to e₀ f/m ≈ 40/37 (1840), 48/45 (1900), 74/70 (1950), 81/77 (2024).
const MORT_ANCHORS: [(i32, [f64; 4]); 6] = [
    (1840, [42.0, 22.0, 11.0, 2.3]),
    (1900, [28.0, 17.0, 8.5, 1.9]),
    (1950, [5.0, 3.5, 2.4, 1.45]),
    (2000, [1.3, 1.15, 1.2, 1.18]),
    (2024, [1.0, 1.0, 1.0, 1.0]),
    (2100, [0.5, 0.7, 0.7, 0.72]),
];

/// The multipliers at `year`, interpolated log-linearly between anchors
/// (geometric interpolation; clamped outside the anchor range).
fn mort_multipliers(year: i32) -> [f64; 4] {
    let last = MORT_ANCHORS.len() - 1;
    let i = MORT_ANCHORS
        .iter()
        .position(|&(y, _)| year <= y)
        .unwrap_or(last + 1);
    if i == 0 {
        return MORT_ANCHORS[0].1;
    }
    if i > last {
        return MORT_ANCHORS[last].1;
    }
    let ((y0, m0), (y1, m1)) = (MORT_ANCHORS[i - 1], MORT_ANCHORS[i]);
    let f = (year - y0) as f64 / (y1 - y0) as f64;
    let mut out = [0.0; 4];
    for k in 0..4 {
        out[k] = exp((1.0 - f) * ln(m0[k]) + f * ln(m1[k]));
    }
    out
}

/// Siler parameters `(infant, background, old-age level, old-age slope)`
/// for `sex` in `year`: hazard `infant·e^{−5x} + background + old·e^{slope·x}`.
fn siler(sex: Sex, year: i32) -> (f64, f64, f64, f64) {
    let [infant, bf, bm, old] = mort_multipliers(year);
    match sex {
        Sex::Female => (infant * 0.030, bf * 0.0005, old * 2.3e-5, 0.092),
        Sex::Male => (infant * 0.030, bm * 0.0011, old * 4.4e-5, 0.087),
    }
}

/// Force of mortality at exact `age` in calendar `year`.
pub fn mortality_hazard(sex: Sex, age: f64, year: i32) -> f64 {
    let (i, b, o, s) = siler(sex, year);
    i * exp(-5.0 * age) + b + o * exp(s * age)
}

/// Probability of dying within age `[a, a + 1)` during calendar year `year`:
/// `1 − exp(−H)` with `H` the hazard integrated over the year of age, in
/// closed form (each Siler term integrates exactly).
pub fn death_prob(sex: Sex, age: u32, year: i32) -> f64 {
    let (i, b, o, s) = siler(sex, year);
    let a = age as f64;
    let infant = i * (exp(-5.0 * a) - exp(-5.0 * (a + 1.0))) / 5.0;
    let old = o * (exp(s * (a + 1.0)) - exp(s * a)) / s;
    1.0 - exp(-(infant + b + old))
}

/// Maximum modelled age.
pub const MAX_AGE: u32 = 110;

// --- unions ------------------------------------------------------------------

/// Median age at first union (including cohabitation), women, by year.
fn median_first_union_f(year: i32) -> f64 {
    interp(
        &[
            (1840, 23.0),
            (1900, 22.5),
            (1940, 21.5),
            (1956, 20.3),
            (1970, 20.8),
            (1990, 23.0),
            (2024, 25.5),
            (2100, 26.5),
        ],
        year,
    )
}

/// Share of a cohort ever entering a union by 50, by year.
fn ever_partnered(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.92),
            (1970, 0.93),
            (2000, 0.89),
            (2024, 0.86),
            (2100, 0.85),
        ],
        year,
    )
}

/// Annual first-union hazard at `age` for the never-partnered, by year. Men
/// run 2.2 years later than women. Log-logistic age distribution (shape 6)
/// among those who ever partner.
pub fn first_union_hazard(sex: Sex, age: u32, year: i32) -> f64 {
    if !(16..=65).contains(&age) {
        return 0.0;
    }
    let median = median_first_union_f(year) + if sex == Sex::Male { 2.2 } else { 0.0 };
    let e = ever_partnered(year);
    let shape = 6.0;
    // Log-logistic CDF relative to age 15.
    let cdf = |a: f64| {
        let x = (a - 15.0).max(0.0) / (median - 15.0);
        let xs = procedural_core::dmath::pow(x, shape);
        xs / (1.0 + xs)
    };
    let (f0, f1) = (cdf(age as f64), cdf(age as f64 + 1.0));
    let surv = 1.0 - e * f0;
    if surv <= 1e-9 {
        return 0.0;
    }
    (e * (f1 - f0) / surv).clamp(0.0, 1.0)
}

/// Share of each block's desired unions that goes to the national
/// (cross-region) market rather than its region's local one, by year
/// (R1 plan, R1b-1).
///
/// **Provisional.** Its meaning depends on how fine the lineage regions are,
/// which waits on the residence decision. The calibration target is the
/// share of couples whose partners were born in different regions (IPUMS
/// birthplace of spouses); the shape (rising with mobility) is what the
/// prototype needs. With two similar-sized regions about half of national
/// unions cross regions.
pub fn national_market_share(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.20),
            (1900, 0.25),
            (1950, 0.35),
            (2020, 0.50),
            (2100, 0.50),
        ],
        year,
    )
}

// --- immigration ---------------------------------------------------------------

/// Youngest and oldest age at arrival for immigrants arriving on their own
/// (R1b-2 step A: adults only; minors arrive with their parents in step B).
pub const MIN_ARRIVAL_AGE: i32 = 18;
pub const MAX_ARRIVAL_AGE: i32 = 80;

/// Net **adult** immigrants per year as a share of the living population;
/// the children of arriving couples come on top (about 15% of all
/// arrivals, as in the historical flows). Prototype anchors shaped on US
/// history (DHS Yearbook, Table 1, net of the heavy return migration before
/// 1914, scaled by 0.85 for adults): the famine-era surge of the 1840s–50s,
/// mass migration to 1914, the 1924 quotas, near zero in the Depression and
/// war, the 1965 reopening, and the 1990–2007 high. Steps are encoded as
/// adjacent anchors. Tuned so the foreign-born share of the living tracks
/// the Census (see `tests/realism.rs`).
pub fn immigration_rate(year: i32) -> f64 {
    0.85 * interp(
        &[
            (1840, 0.0110),
            (1855, 0.0100),
            (1860, 0.0065),
            (1880, 0.0065),
            (1914, 0.0058),
            (1915, 0.0020),
            (1929, 0.0018),
            (1930, 0.0003),
            (1945, 0.0003),
            (1946, 0.0012),
            (1965, 0.0012),
            (1966, 0.0025),
            (1990, 0.0033),
            (2007, 0.0042),
            (2008, 0.0032),
            (2100, 0.0030),
        ],
        year,
    )
}

/// Relative weight of arriving at `age`: the labour component of the
/// Rogers–Castro model migration schedule (Rogers & Castro 1981), peak near
/// 23, with the standard `μ = 20`, `α = 0.1`, `λ = 0.4`.
pub fn arrival_age_weight(age: i32) -> f64 {
    if !(MIN_ARRIVAL_AGE..=MAX_ARRIVAL_AGE).contains(&age) {
        return 0.0;
    }
    let x = (age - 20) as f64;
    exp(-0.1 * x - exp(-0.4 * x))
}

/// Share of adult immigrants who arrive as a couple (with each other), by
/// arrival year. **Provisional**, tuned so that children of arriving
/// couples are about 15% of arrivals: such couples bring two to three
/// children each, and many married immigrants came ahead alone (chain
/// migration). Lowest in the male-heavy mass migration before 1914, higher
/// after the 1920s quotas and the 1965 family-reunification law.
pub fn couple_arrival_share(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.14),
            (1900, 0.10),
            (1930, 0.18),
            (1970, 0.20),
            (2100, 0.18),
        ],
        year,
    )
}

/// Oldest age at which a child arrives with the parents; older children of
/// arriving couples stay abroad.
pub const MAX_CHILD_ARRIVAL_AGE: i32 = 17;

/// Youngest age of a woman arriving in a couple: with husbands at most
/// [`ARRIVAL_GAP_MIN`] years younger, both partners were at least 16 when
/// the union started.
pub const MIN_COUPLE_ARRIVAL_AGE: i32 = 19;

/// Most years a husband in an arriving couple may be younger than his wife.
pub const ARRIVAL_GAP_MIN: i32 = -3;

/// Share of immigrants who are men, by arrival year: male-heavy mass
/// migration before 1914, female majorities from the 1930s to the 1980s.
pub fn immigrant_male_share(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.58),
            (1910, 0.62),
            (1930, 0.45),
            (1980, 0.47),
            (2000, 0.50),
            (2100, 0.50),
        ],
        year,
    )
}

/// Share of each sex's first-union demand that goes to same-sex unions,
/// by year. **Provisional:** calibrated only to about 1.5% of coupled
/// households being same-sex couples in 2019 (ACS); earlier partnerships
/// were largely unrecorded, and the rise follows recorded prevalence.
pub fn same_sex_share(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.004),
            (1960, 0.005),
            (1990, 0.012),
            (2010, 0.025),
            (2100, 0.030),
        ],
        year,
    )
}

/// Age-gap kernel for same-sex couples: the opposite-sex kernel made
/// symmetric (neither partner is the "older" side by convention).
pub fn same_sex_gap_weight(gap: i32) -> f64 {
    0.5 * (age_gap_weight(gap) + age_gap_weight(-gap))
}

/// Age-gap kernel `g(age_m − age_f)` from CPS 2023 (FG3) married couples:
/// 35% within a year, husband 2+ older in 50%, wife 2+ older in 14%.
pub fn age_gap_weight(gap: i32) -> f64 {
    match gap {
        -1..=1 => [0.09, 0.172, 0.092][(gap + 1) as usize],
        2 | 3 => 0.10,
        4 | 5 => 0.061,
        6..=9 => 0.028,
        10..=14 => 0.0092,
        15..=19 => 0.0026,
        20..=30 => 0.0008,
        -3 | -2 => 0.035,
        -5 | -4 => 0.0165,
        -9..=-6 => 0.00625,
        -19..=-10 => 0.0012,
        _ => 0.0,
    }
}

// --- fertility ---------------------------------------------------------------

/// Parity pmf (0..=8) among women in a union, by union year. Anchors trace
/// US fertility eras: high (1840), transition (1900), Depression low
/// (1935), baby boom (1957), bust (1976), modern (2024).
pub fn union_parity_pmf(year: i32) -> [f64; 9] {
    const ANCHORS: [(i32, [f64; 9]); 6] = [
        (1840, [0.06, 0.05, 0.07, 0.09, 0.11, 0.12, 0.13, 0.12, 0.25]),
        (1900, [0.08, 0.10, 0.14, 0.16, 0.15, 0.12, 0.10, 0.07, 0.08]),
        (1935, [0.10, 0.18, 0.28, 0.20, 0.12, 0.06, 0.03, 0.02, 0.01]),
        (1957, [0.05, 0.07, 0.18, 0.25, 0.20, 0.12, 0.07, 0.03, 0.03]),
        (
            1976,
            [0.09, 0.17, 0.39, 0.22, 0.08, 0.03, 0.01, 0.005, 0.005],
        ),
        (
            2024,
            [0.10, 0.20, 0.38, 0.20, 0.08, 0.03, 0.005, 0.0025, 0.0025],
        ),
    ];
    let mut out = [0.0; 9];
    for (k, o) in out.iter_mut().enumerate() {
        let a: Vec<(i32, f64)> = ANCHORS.iter().map(|&(y, p)| (y, p[k])).collect();
        *o = interp(&a, year);
    }
    out
}

/// First-birth offsets in years after union start (1..=4), by era.
pub fn first_birth_offset_pmf(year: i32) -> [f64; 4] {
    let modern = ((year - 1950) as f64 / 70.0).clamp(0.0, 1.0);
    let hist = [0.60, 0.28, 0.08, 0.04];
    let now = [0.35, 0.30, 0.20, 0.15];
    let mut out = [0.0; 4];
    for k in 0..4 {
        out[k] = hist[k] * (1.0 - modern) + now[k] * modern;
    }
    out
}

/// Birth spacing patterns (years between births): tight, medium, loose.
pub const SPACING: [[u32; 2]; 3] = [[1, 2], [2, 3], [3, 5]];
/// Weights of the spacing patterns.
pub const SPACING_PMF: [f64; 3] = [0.30, 0.45, 0.25];

/// Latest age at which a birth is planned.
pub const MAX_BIRTH_AGE: i32 = 45;
/// Earliest age at which a birth is planned.
pub const MIN_BIRTH_AGE: i32 = 15;
/// Days from conception to birth (38 weeks). A father need only be alive at
/// conception: a child born within this long after his death is still his.
pub const GESTATION_DAYS: i64 = 266;

// --- dissolution ---------------------------------------------------------------

/// Dissolution bands (years after union start): `[lo, hi)`; `None` means the
/// union lasts until a partner dies.
pub const DISSOLUTION_BANDS: [Option<(u32, u32)>; 6] = [
    None,
    Some((1, 4)),
    Some((4, 8)),
    Some((8, 13)),
    Some((13, 21)),
    Some((21, 41)),
];

/// Probability a union formed in `year` ends in separation (not death).
fn divorce_share(year: i32) -> f64 {
    interp(
        &[
            (1840, 0.05),
            (1900, 0.10),
            (1950, 0.28),
            (1970, 0.50),
            (2000, 0.45),
            (2024, 0.40),
            (2100, 0.40),
        ],
        year,
    )
}

/// Pmf over [`DISSOLUTION_BANDS`] for a union formed in `year`. Durations
/// of dissolving unions follow NSFG's early-peaking hazard: about a third
/// within 4 years, median about 8.
pub fn dissolution_pmf(year: i32) -> [f64; 6] {
    let d = divorce_share(year);
    let within = [0.18, 0.24, 0.24, 0.20, 0.14];
    let mut out = [0.0; 6];
    out[0] = 1.0 - d;
    for k in 0..5 {
        out[k + 1] = d * within[k];
    }
    out
}

/// Share of women with non-union births (0, 1 or 2 births), by year of
/// the woman's 25th birthday. Rises from rare to about 12% today.
pub fn nonunion_count_pmf(year: i32) -> [f64; 3] {
    let p1 = interp(
        &[
            (1840, 0.025),
            (1950, 0.03),
            (1980, 0.08),
            (2024, 0.11),
            (2100, 0.11),
        ],
        year,
    );
    let p2 = p1 * 0.25;
    [1.0 - p1 - p2, p1, p2]
}

/// Age pmf (15..=44) of a non-union birth: peaks in the early twenties.
pub fn nonunion_age_weight(age: i32) -> f64 {
    if !(MIN_BIRTH_AGE..MAX_BIRTH_AGE).contains(&age) {
        return 0.0;
    }
    let x = (age - 21) as f64;
    exp(-x * x / (2.0 * 36.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period_e0(sex: Sex, year: i32) -> f64 {
        let (mut l, mut e) = (1.0, 0.0);
        for a in 0..=MAX_AGE {
            let q = death_prob(sex, a, year);
            e += l * (1.0 - q / 2.0);
            l *= 1.0 - q;
        }
        e
    }

    #[test]
    fn life_expectancy_follows_anchors() {
        let cases = [(1840, 39.5, 36.8), (1900, 47.5, 44.6), (2024, 81.6, 76.6)];
        for (year, ef, em) in cases {
            let (f, m) = (period_e0(Sex::Female, year), period_e0(Sex::Male, year));
            assert!((f - ef).abs() < 1.0, "{year} female e0 {f}");
            assert!((m - em).abs() < 1.0, "{year} male e0 {m}");
        }
    }

    #[test]
    fn death_prob_is_the_exact_hazard_integral() {
        // Compare the closed form with a fine midpoint rule.
        for sex in [Sex::Female, Sex::Male] {
            for year in [1840, 1923, 2024, 2100] {
                for age in [0u32, 1, 30, 70, 100, 110] {
                    let n = 20_000;
                    let h: f64 = (0..n)
                        .map(|k| {
                            mortality_hazard(sex, age as f64 + (k as f64 + 0.5) / n as f64, year)
                        })
                        .sum::<f64>()
                        / n as f64;
                    let q = 1.0 - exp(-h);
                    let got = death_prob(sex, age, year);
                    assert!(
                        (got - q).abs() < 1e-7 * q.max(1e-3),
                        "{sex:?} {year} {age}: {got} vs {q}"
                    );
                }
            }
        }
    }

    #[test]
    fn first_union_hazard_hits_median_and_ever_share() {
        for year in [1900, 1956, 2024] {
            let mut never = 1.0;
            let mut median = None;
            for age in 0..=65u32 {
                never *= 1.0 - first_union_hazard(Sex::Female, age, year);
                if median.is_none() && never <= 1.0 - 0.5 * ever_partnered(year) {
                    median = Some(age);
                }
            }
            let target = median_first_union_f(year);
            let m = median.unwrap() as f64;
            assert!((m - target).abs() <= 1.5, "{year}: median {m} vs {target}");
            assert!(
                ((1.0 - never) - ever_partnered(year)).abs() < 0.02,
                "{year}: ever {}",
                1.0 - never
            );
        }
    }

    #[test]
    fn pmfs_are_normalised() {
        for year in [1840, 1900, 1957, 2024, 2100] {
            let s: f64 = union_parity_pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9, "{year} parity sum {s}");
            let s: f64 = dissolution_pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
            let s: f64 = first_birth_offset_pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
            let s: f64 = nonunion_count_pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
        }
        let g: f64 = (-40..=40).map(age_gap_weight).sum();
        assert!((g - 1.0).abs() < 0.03, "gap kernel mass {g}");
    }
}
