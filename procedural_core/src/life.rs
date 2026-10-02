//! Life tables and hazards: survival from discrete hazards, the Siler
//! mortality model, first-event densities, defective ("cure") hazards, and
//! inverse-CDF draws of an age conditioned on survival.
//!
//! Ages are whole years `0, 1, 2, …`; a discrete hazard `h(a)` is the
//! probability of the event during age `a` given none before. Survival to
//! age `n` is `S(n) = Π_{a<n} (1 − h(a))`, multiplied left to right from 1.
//! All transcendental math goes through [`crate::dmath`].

use crate::dmath::exp;
use crate::table::CoarseRow;

/// Survival to age `n`: `Π_{a<n} (1 − h(a))`, from 1 (1 for `n ≤ 0`).
#[inline]
pub fn survival(n: i32, mut h: impl FnMut(u32) -> f64) -> f64 {
    (0..n.max(0)).fold(1.0, |s, a| s * (1.0 - h(a as u32)))
}

/// The survivorship column `l(0..=max)` of a life table from death
/// probabilities `q(a)`, plus a closing `0` (so `l` has `max + 2` entries
/// and `l(a) − l(a + 1)` is the deaths at age `a`, everyone dying by
/// `max`). Appends to `out`.
pub fn survivorship(max: u32, mut q: impl FnMut(u32) -> f64, out: &mut Vec<f64>) {
    let mut s = 1.0;
    for a in 0..=max {
        out.push(s);
        s *= 1.0 - q(a);
    }
    out.push(0.0);
}

/// The first-event probabilities `S(a)·h(a)` for ages `0..=max`, where
/// `S` is survival under the discrete hazard `h`.
pub fn first_event_pmf(max: i32, mut h: impl FnMut(i32) -> f64) -> Vec<f64> {
    let mut never = 1.0;
    (0..=max)
        .map(|a| {
            let ha = h(a);
            let d = never * ha;
            never *= 1.0 - ha;
            d
        })
        .collect()
}

/// The cumulative incidence `1 − S(a)` (the share who had the event before
/// age `a`) for ages `0..=max`.
pub fn cumulative_incidence(max: i32, mut h: impl FnMut(i32) -> f64) -> Vec<f64> {
    let mut never = 1.0;
    (0..=max)
        .map(|a| {
            let e = 1.0 - never;
            never *= 1.0 - h(a);
            e
        })
        .collect()
}

/// The discrete hazard of a defective (mixture-cure) distribution: a share
/// `ever` of people have the event, at ages with CDF `F`; given none by age
/// `a`, the event happens during `[a, a + 1)` with probability
/// `ever·(F(a+1) − F(a)) / (1 − ever·F(a))`, given `f0 = F(a)` and
/// `f1 = F(a + 1)`. Zero once survival is below `1e-9`; clamped to `[0, 1]`.
#[inline]
pub fn cure_hazard(ever: f64, f0: f64, f1: f64) -> f64 {
    let surv = 1.0 - ever * f0;
    if surv <= 1e-9 {
        return 0.0;
    }
    (ever * (f1 - f0) / surv).clamp(0.0, 1.0)
}

/// A Siler mortality hazard: `infant·e^(−decay·x) + background + old·e^(slope·x)`
/// at exact age `x` (an infant term falling with age, a constant, and a
/// Gompertz old-age term).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Siler {
    pub infant: f64,
    pub decay: f64,
    pub background: f64,
    pub old: f64,
    pub slope: f64,
}

impl Siler {
    /// The force of mortality at exact age `x`.
    #[inline]
    pub fn hazard(&self, x: f64) -> f64 {
        self.infant * exp(-self.decay * x) + self.background + self.old * exp(self.slope * x)
    }

    /// The probability of dying within ages `[a, a + 1)`: `1 − exp(−H)` with
    /// `H` the hazard integrated over the year of age, in closed form (each
    /// term integrates exactly).
    #[inline]
    pub fn year_death_prob(&self, age: u32) -> f64 {
        let a = age as f64;
        let k = self.decay;
        let s = self.slope;
        let infant = self.infant * (exp(-k * a) - exp(-k * (a + 1.0))) / k;
        let old = self.old * (exp(s * (a + 1.0)) - exp(s * a)) / s;
        1.0 - exp(-(infant + self.background + old))
    }
}

/// The weight of age `a` in a stable population growing at rate `growth`:
/// `e^(−growth·a)` (Lotka; survival comes separately).
#[inline]
pub fn stable_age_weight(growth: f64, age: f64) -> f64 {
    exp(-growth * age)
}

/// Add `scale · (l(a) − l(a + 1)) / l(from)` to `out[a]` for every age
/// `a ≥ from`: the expected deaths by age of `scale` people alive at
/// `from` (a negative `scale` removes them). Nothing if `l(from) ≤ 0`.
/// `l` needs one more entry than `out`.
pub fn add_conditional_deaths(l: &[f64], from: usize, scale: f64, out: &mut [f64]) {
    if from >= out.len() || l[from] <= 0.0 {
        return;
    }
    for (a, d) in out.iter_mut().enumerate().skip(from) {
        *d += scale * (l[a] - l[a + 1]) / l[from];
    }
}

/// Age at death from a survivorship row `l` (non-increasing, ending in 0,
/// so ages are `0..=l.len() − 2`) conditioned on reaching `req`, by
/// inversion of `u ∈ (0, 1]`: the last age whose survivors are at least
/// `u · l(req)`.
#[inline]
pub fn invert_survival(l: CoarseRow<f64>, req: usize, u: f64) -> usize {
    let max = l.row.len() - 2;
    let target = u * l.row[req];
    (l.first_fail(req + 1, |s| s >= target) - 1).min(max)
}

/// Age at death from a cumulative row (`cum(a)`: mass below age `a`,
/// non-decreasing, ages `0..=cum.len() − 2`) conditioned on reaching
/// `req`, by inversion of `u ∈ [0, 1)`: the last age whose mass below is at
/// most the target. `req` itself if no mass lies past it.
#[inline]
pub fn invert_cumulative(cum: CoarseRow<f64>, req: usize, u: f64) -> usize {
    let max = cum.row.len() - 2;
    let tail = cum.row[max + 1] - cum.row[req];
    if tail <= 0.0 {
        return req;
    }
    let threshold = cum.row[req] + u * tail;
    (cum.first_fail(req + 1, |c| c <= threshold) - 1).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;
    use crate::table::coarse_index;

    fn siler() -> Siler {
        Siler {
            infant: 0.05,
            decay: 1.2,
            background: 0.004,
            old: 3e-5,
            slope: 0.095,
        }
    }

    #[test]
    fn year_death_prob_is_the_hazard_integral() {
        let s = siler();
        for age in [0u32, 1, 5, 30, 70, 100] {
            // Midpoint rule on a fine grid.
            let steps = 20_000;
            let h: f64 = (0..steps)
                .map(|i| s.hazard(age as f64 + (i as f64 + 0.5) / steps as f64) / steps as f64)
                .sum();
            let want = 1.0 - (-h).exp();
            assert!((s.year_death_prob(age) - want).abs() < 1e-9, "{age}");
        }
    }

    #[test]
    fn survival_tables_agree() {
        let s = siler();
        let mut l = Vec::new();
        survivorship(110, |a| s.year_death_prob(a), &mut l);
        assert_eq!(l.len(), 112);
        assert_eq!(l[0], 1.0);
        assert_eq!(*l.last().unwrap(), 0.0);
        assert!(l.windows(2).all(|w| w[0] >= w[1]));
        for n in [0, 1, 17, 65, 110] {
            let direct = survival(n, |a| s.year_death_prob(a));
            assert_eq!(direct.to_bits(), l[n as usize].to_bits(), "{n}");
        }
        assert_eq!(survival(-3, |_| 0.5), 1.0);
    }

    #[test]
    fn first_event_pmf_and_incidence_add_up() {
        let h = |a: i32| if a < 15 { 0.0 } else { 0.08 };
        let pmf = first_event_pmf(60, h);
        let inc = cumulative_incidence(60, h);
        let mut acc = 0.0;
        for a in 0..=60 {
            assert!((inc[a] - acc).abs() < 1e-12, "{a}");
            acc += pmf[a];
        }
        assert!(pmf[..15].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn cure_hazard_reproduces_the_defective_distribution() {
        // CDF F(a) = 1 - 0.9^a among those who ever have the event.
        let ever = 0.8;
        let f = |a: f64| 1.0 - 0.9f64.powf(a);
        let mut s = 1.0;
        for a in 0..80 {
            let h = cure_hazard(ever, f(a as f64), f(a as f64 + 1.0));
            s *= 1.0 - h;
            let want = 1.0 - ever * f(a as f64 + 1.0);
            assert!((s - want).abs() < 1e-12, "{a}");
        }
        assert_eq!(cure_hazard(1.0, 1.0, 1.0), 0.0);
    }

    #[test]
    fn conditional_deaths_sum_to_the_people() {
        let s = siler();
        let mut l = Vec::new();
        survivorship(110, |a| s.year_death_prob(a), &mut l);
        let mut out = vec![0.0; 111];
        add_conditional_deaths(&l, 40, 1000.0, &mut out);
        assert!(out[..40].iter().all(|&x| x == 0.0));
        assert!((out.iter().sum::<f64>() - 1000.0).abs() < 1e-9);
        add_conditional_deaths(&l, 40, -1000.0, &mut out);
        assert!(out.iter().all(|&x| x.abs() < 1e-12));
    }

    #[test]
    fn inversions_follow_the_conditional_law() {
        let s = siler();
        let mut l = Vec::new();
        survivorship(110, |a| s.year_death_prob(a), &mut l);
        let coarse: Vec<f64> = coarse_index(&l).collect();
        let row = CoarseRow {
            row: &l,
            coarse: &coarse,
        };
        let mut cum = vec![0.0];
        let mut acc = 0.0;
        for a in 0..=110 {
            acc += l[a] - l[a + 1];
            cum.push(acc);
        }
        let cc: Vec<f64> = coarse_index(&cum).collect();
        let crow = CoarseRow {
            row: &cum,
            coarse: &cc,
        };
        let req = 30;
        let trials = 200_000u64;
        let mut hist_s = vec![0u64; 111];
        let mut hist_c = vec![0u64; 111];
        for seed in 0..trials {
            let k = Key::from_seed(seed);
            let a = invert_survival(row, req, k.unit_open0());
            assert!(a >= req);
            hist_s[a] += 1;
            hist_c[invert_cumulative(crow, req, k.unit())] += 1;
        }
        for a in req..=110 {
            let want = (l[a] - l[a + 1]) / l[req];
            for h in [&hist_s, &hist_c] {
                let got = h[a] as f64 / trials as f64;
                assert!(
                    (got - want).abs() < 4.0 * (want / trials as f64).sqrt() + 1e-4,
                    "{a}"
                );
            }
        }
    }

    #[test]
    fn golden() {
        let s = siler();
        let bits = [
            s.hazard(0.5),
            s.year_death_prob(0),
            s.year_death_prob(45),
            s.year_death_prob(90),
            cure_hazard(0.9, 0.3, 0.42),
            stable_age_weight(0.02, 37.0),
        ]
        .map(f64::to_bits);
        assert_eq!(bits, GOLDEN);
    }

    const GOLDEN: [u64; 6] = [
        4584696420171128285,
        4584859695621392512,
        4573847237325187712,
        4594697189395021056,
        4594498308068683518,
        4602266540126521155,
    ];
}
