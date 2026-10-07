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

impl Siler {
    /// The hazard integrated over ages `[0, x]`, in closed form.
    #[inline]
    pub fn cumulative_hazard(&self, x: f64) -> f64 {
        self.infant / self.decay * (1.0 - exp(-self.decay * x)) + self.background * x + self.old / self.slope * (exp(self.slope * x) - 1.0)
    }

    /// The age, from `lo` up (at most 150), at which the cumulative hazard
    /// reaches `target`: Newton steps from a Gompertz guess, safeguarded by
    /// bisection.
    pub fn age_at_cumulative_hazard(&self, target: f64, lo: f64) -> f64 {
        let (mut a, mut b) = (lo, 150.0);
        if self.cumulative_hazard(b) <= target {
            return b;
        }
        let mut x = (crate::dmath::ln(1.0 + (target * self.slope / self.old).max(0.0)) / self.slope).clamp(a, b);
        for _ in 0..40 {
            // The cumulative hazard and the hazard share their two
            // exponentials (the same operations as `cumulative_hazard` and
            // `hazard`, so the same bits).
            let (e1, e2) = (exp(-self.decay * x), exp(self.slope * x));
            let f = self.infant / self.decay * (1.0 - e1) + self.background * x + self.old / self.slope * (e2 - 1.0) - target;
            if f.abs() < 1e-10 {
                break;
            }
            if f > 0.0 {
                b = x
            } else {
                a = x
            }
            let n = x - f / (self.infant * e1 + self.background + self.old * e2);
            x = if n > a && n < b { n } else { 0.5 * (a + b) };
        }
        x
    }

    /// An age at death by inversion of `u ∈ [0, 1)`, conditioned on reaching
    /// `from` and, when given, dying before `to`.
    pub fn conditional_death_age(&self, from: f64, to: Option<f64>, u: f64) -> f64 {
        let h0 = self.cumulative_hazard(from);
        let span = to.map_or(1.0, |t| 1.0 - exp(h0 - self.cumulative_hazard(t)));
        self.age_at_cumulative_hazard(h0 - crate::dmath::ln(1.0 - u * span), from)
    }
}

/// Of a population whose share `ever` has the event at ages with CDF value
/// `cdf` by now, the share still to have it among those without it yet:
/// `(ever − ever·cdf)/(1 − ever·cdf)`, clamped to `[0, 1]`.
#[inline]
pub fn remaining_share(ever: f64, cdf: f64) -> f64 {
    let f = ever * cdf;
    ((ever - f) / (1.0 - f)).clamp(0.0, 1.0)
}

impl Siler {
    /// Survival to exact age `x`: `exp(−H(x))`.
    #[inline]
    pub fn survival(&self, x: f64) -> f64 {
        exp(-self.cumulative_hazard(x))
    }
}

/// A cumulative hazard tabulated at ages `0, step, 2·step, …` and linear
/// between them (beyond the last age, the last slope continues): a survival
/// law whose inverse is exact and cheap. Built from a [`Siler`] it agrees
/// with it at every knot; between knots the hazard is constant (the
/// piecewise-exponential law). [`HazardTable::age_at`] inverts
/// [`HazardTable::cumulative`] exactly (up to float rounding) with one
/// binary search over the knots, so a world can draw ages at death without
/// Newton steps and count them consistently.
#[derive(Clone, Debug)]
pub struct HazardTable {
    step: f64,
    h: Vec<f64>,
    /// For bucket `b` of `g = H/(1 + H)` (in `[0, 1)`, [`BUCKETS`] buckets):
    /// the last knot whose `H` is at or below the bucket's start. The
    /// segment holding a target lies between `index[b]` and `index[b + 1]`,
    /// so [`Self::age_at`] searches a few knots instead of all of them.
    index: Vec<u16>,
}

/// Buckets of [`HazardTable`]'s index.
const BUCKETS: usize = 1024;

impl HazardTable {
    /// `s`'s cumulative hazard at every `step` from 0 to `max_age`.
    pub fn from_siler(s: &Siler, max_age: f64, step: f64) -> Self {
        let n = (max_age / step).ceil() as usize + 1;
        let mut h: Vec<f64> = (0..n).map(|i| s.cumulative_hazard(i as f64 * step)).collect();
        // Strictly increasing, so the inverse is a function.
        for i in 1..n {
            if h[i] <= h[i - 1] {
                h[i] = h[i - 1] + 1e-12;
            }
        }
        let index = (0..=BUCKETS)
            .map(|b| {
                // The start of bucket `b` in H: g = b/B ⟺ H = g/(1 − g).
                let g = b as f64 / BUCKETS as f64;
                let hb = if b == BUCKETS { f64::INFINITY } else { g / (1.0 - g) };
                (h.partition_point(|&v| v <= hb).max(1) - 1).min(n - 2) as u16
            })
            .collect();
        Self { step, h, index }
    }

    /// The cumulative hazard at `age` (≥ 0).
    #[inline]
    pub fn cumulative(&self, age: f64) -> f64 {
        let x = (age / self.step).max(0.0);
        let last = self.h.len() - 1;
        let i = (x.floor() as usize).min(last - 1);
        let f = x - i as f64;
        self.h[i] + f * (self.h[i + 1] - self.h[i])
    }

    /// Survival to `age`.
    #[inline]
    pub fn survival(&self, age: f64) -> f64 {
        exp(-self.cumulative(age))
    }

    /// The age where the cumulative hazard reaches `target` (≥ 0).
    #[inline]
    pub fn age_at(&self, target: f64) -> f64 {
        let last = self.h.len() - 1;
        // The segment holding `target`: h[i] ≤ target < h[i+1] (the last
        // segment extends beyond the table). The bucket of `target` bounds
        // it: knots in [index[b], index[b + 1] + 1] (the same `i` as a search
        // over all knots).
        let g = target / (1.0 + target);
        let b = ((g.max(0.0) * BUCKETS as f64) as usize).min(BUCKETS - 1);
        let (lo, hi) = (self.index[b] as usize, (self.index[b + 1] as usize + 1).min(last));
        let i = (lo + self.h[lo..=hi].partition_point(|&v| v <= target)).max(1) - 1;
        let i = i.min(last - 1);
        let (a, b) = (self.h[i], self.h[i + 1]);
        (i as f64 + (target - a) / (b - a)) * self.step
    }

    /// An age at death conditioned on reaching `from` (and dying before
    /// `to`, when given), at quantile `u`: as [`Siler::conditional_death_age`].
    #[inline]
    pub fn conditional_death_age(&self, from: f64, to: Option<f64>, u: f64) -> f64 {
        let h0 = self.cumulative(from);
        let span = to.map_or(1.0, |t| 1.0 - exp(h0 - self.cumulative(t)));
        let a = self.age_at(h0 - crate::dmath::ln(1.0 - u * span));
        match to {
            Some(t) => a.clamp(from, t),
            None => a.max(from),
        }
    }

    /// [`Self::conditional_death_age`] with no upper bound, given
    /// `h0 = cumulative(from)` (store it when `from` repeats): the same
    /// result.
    #[inline]
    pub fn death_age_given(&self, from: f64, h0: f64, u: f64) -> f64 {
        self.age_at(h0 - crate::dmath::ln(1.0 - u)).max(from)
    }

    /// Remaining life expectancy at `from`: `∫ S(x)/S(from) dx` over
    /// `x ≥ from`, exact for the piecewise-exponential law (each segment's
    /// constant hazard integrated in closed form; the last segment's hazard
    /// continues beyond the table).
    pub fn remaining_life(&self, from: f64) -> f64 {
        let last = self.h.len() - 1;
        let h0 = self.cumulative(from);
        // ∫ over [0, d] of e^{−(ha − h0) − m·x}: (1 − e^{−m d})/m, or d for
        // a negligible m·d (the series' error is below 1e-16 relative).
        let seg = |ha: f64, m: f64, d: f64| {
            let s0 = exp(h0 - ha);
            if m * d < 1e-8 { s0 * d * (1.0 - 0.5 * m * d) } else { s0 * (1.0 - exp(-m * d)) / m }
        };
        let slope = |i: usize| (self.h[i + 1] - self.h[i]) / self.step;
        let from = from.max(0.0);
        let end = last as f64 * self.step;
        let mut total = 0.0;
        if from < end {
            let i = ((from / self.step).floor() as usize).min(last - 1);
            total += seg(h0, slope(i), (i + 1) as f64 * self.step - from);
            for j in i + 1..last {
                total += seg(self.h[j], slope(j), self.step);
            }
        }
        // Beyond the table: the last hazard forever.
        let m = slope(last - 1);
        total + exp(h0 - self.cumulative(from.max(end))) / m
    }

    /// Heap bytes.
    pub fn heap_bytes(&self) -> usize {
        self.h.capacity() * 8 + self.index.capacity() * 2
    }
}

#[cfg(test)]
mod hazard_table_tests {
    use super::*;

    fn siler() -> Siler {
        Siler { infant: 0.05, decay: 1.5, background: 0.002, old: 3e-5, slope: 0.095 }
    }

    #[test]
    fn agrees_with_siler_at_knots_and_inverts() {
        let s = siler();
        let t = HazardTable::from_siler(&s, 130.0, 0.25);
        for i in 0..520 {
            let a = i as f64 * 0.25;
            assert!((t.cumulative(a) - s.cumulative_hazard(a)).abs() < 1e-9 * (1.0 + s.cumulative_hazard(a)));
        }
        for k in 0..2000 {
            let a = k as f64 * 0.0617;
            let h = t.cumulative(a);
            assert!((t.age_at(h) - a).abs() < 1e-6, "{a}");
        }
    }

    #[test]
    fn indexed_inverse_equals_the_full_search() {
        let t = HazardTable::from_siler(&siler(), 130.0, 0.25);
        let full = |target: f64| {
            let last = t.h.len() - 1;
            let i = (t.h.partition_point(|&v| v <= target).max(1) - 1).min(last - 1);
            let (a, b) = (t.h[i], t.h[i + 1]);
            (i as f64 + (target - a) / (b - a)) * t.step
        };
        let mut k = 0u64;
        for target in (0..200_000).map(|j| j as f64 * 0.00025).chain((0..2000).map(|j| j as f64 * 0.05)).chain(t.h.iter().copied()) {
            k += 1;
            assert_eq!(t.age_at(target).to_bits(), full(target).to_bits(), "{target}");
        }
        assert!(k > 200_000);
    }

    #[test]
    fn conditional_ages_are_monotone_and_close_to_siler() {
        let s = siler();
        let t = HazardTable::from_siler(&s, 130.0, 0.25);
        let mut prev = 0.0;
        for k in 0..10_000 {
            let u = (k as f64 + 0.5) / 10_000.0;
            let a = t.conditional_death_age(16.0, None, u);
            assert!(a >= prev && a >= 16.0);
            prev = a;
            assert!((a - s.conditional_death_age(16.0, None, u)).abs() < 0.05, "{u}: {a}");
        }
        for k in 0..1000 {
            let u = (k as f64 + 0.5) / 1000.0;
            let a = t.conditional_death_age(0.0, Some(16.0), u);
            assert!((0.0..=16.0).contains(&a));
        }
    }

    #[test]
    fn death_age_given_equals_the_conditional_age() {
        let t = HazardTable::from_siler(&siler(), 130.0, 0.25);
        for from in [0.0, 16.0, 16.37, 45.0, 90.0] {
            let h0 = t.cumulative(from);
            for k in 0..5000 {
                let u = (k as f64 + 0.5) / 5000.0;
                assert_eq!(t.death_age_given(from, h0, u).to_bits(), t.conditional_death_age(from, None, u).to_bits());
            }
        }
    }

    #[test]
    fn remaining_life_is_the_survival_integral() {
        // A constant hazard: exactly 1/λ from any age, inside or beyond the table.
        let flat = Siler { infant: 0.0, decay: 1.0, background: 0.02, old: 0.0, slope: 1.0 };
        let t = HazardTable::from_siler(&flat, 130.0, 0.25);
        for from in [0.0, 16.0, 16.37, 129.9, 140.0] {
            assert!((t.remaining_life(from) - 50.0).abs() < 1e-9, "{from}: {}", t.remaining_life(from));
        }
        // The Siler law: against a fine midpoint rule on the table's own survival.
        let t = HazardTable::from_siler(&siler(), 130.0, 0.25);
        for from in [0.0, 16.0, 16.37, 60.0, 100.0] {
            let (steps, top) = (400_000, 200.0);
            let dx = (top - from) / steps as f64;
            let s0 = t.survival(from);
            let num: f64 = (0..steps).map(|k| t.survival(from + (k as f64 + 0.5) * dx) / s0 * dx).sum();
            assert!((t.remaining_life(from) - num).abs() < 1e-6 * num, "{from}: {} vs {num}", t.remaining_life(from));
        }
        // Remaining life falls with age (for this law, past infancy).
        assert!(t.remaining_life(16.0) > t.remaining_life(60.0));
    }

    #[test]
    fn remaining_life_golden() {
        let t = HazardTable::from_siler(&siler(), 130.0, 0.25);
        assert_eq!(format!("{:.9}", t.remaining_life(16.0)), "58.928043761");
        assert_eq!(format!("{:.9}", t.remaining_life(0.0)), "70.384835881");
    }

    #[test]
    fn golden() {
        let t = HazardTable::from_siler(&siler(), 130.0, 0.25);
        let a = t.conditional_death_age(16.0, None, 0.5);
        assert!((a - siler().conditional_death_age(16.0, None, 0.5)).abs() < 0.05);
        assert_eq!(format!("{a:.6}"), format!("{:.6}", t.conditional_death_age(16.0, None, 0.5)));
    }
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

#[cfg(test)]
mod siler_tests {
    use super::*;

    const S: Siler = Siler { infant: 0.05, decay: 1.2, background: 0.004, old: 0.00004, slope: 0.095 };

    #[test]
    fn cumulative_hazard_integrates_the_hazard() {
        for x in [0.0, 0.5, 3.0, 20.0, 60.0, 95.0] {
            let n = 20_000;
            let dx = x / n as f64;
            let num: f64 = (0..n).map(|i| S.hazard((i as f64 + 0.5) * dx) * dx).sum();
            assert!((S.cumulative_hazard(x) - num).abs() < 1e-6 * (1.0 + num), "{x}");
        }
    }

    #[test]
    fn inversion_round_trips_and_respects_bounds() {
        for lo in [0.0, 16.0, 40.0] {
            for h in [0.01, 0.3, 1.0, 3.0] {
                let target = S.cumulative_hazard(lo) + h;
                let a = S.age_at_cumulative_hazard(target, lo);
                assert!(a >= lo && (S.cumulative_hazard(a) - target).abs() < 1e-8, "{lo} {h}");
            }
        }
        for i in 0..200 {
            let u = i as f64 / 200.0;
            let a = S.conditional_death_age(14.3, Some(16.0), u);
            assert!((14.3..=16.0).contains(&a), "{u}: {a}");
            assert!(S.conditional_death_age(30.0, None, u) >= 30.0);
        }
    }

    #[test]
    fn golden() {
        assert_eq!(S.cumulative_hazard(50.0), 0.05 / 1.2 * (1.0 - exp(-60.0)) + 0.2 + 0.00004 / 0.095 * (exp(4.75) - 1.0));
        assert_eq!(format!("{:.6}", S.conditional_death_age(0.0, None, 0.5)), "71.262663");
        assert_eq!(format!("{:.6}", S.conditional_death_age(10.0, Some(16.0), 0.25)), "11.496268");
    }
}

#[cfg(test)]
mod remaining_tests {
    use super::*;

    #[test]
    fn remaining_share_matches_the_conditional() {
        for ever in [0.0, 0.5, 0.9, 1.0] {
            for i in 0..20 {
                let cdf = i as f64 / 20.0;
                let r = remaining_share(ever, cdf);
                // Those still to have it over those without it yet.
                let expect = if ever * cdf >= 1.0 { 0.0 } else { (ever * (1.0 - cdf)) / (1.0 - ever * cdf) };
                assert!((r - expect.clamp(0.0, 1.0)).abs() < 1e-12, "{ever} {cdf}");
            }
        }
    }

    #[test]
    fn golden() {
        assert_eq!(remaining_share(0.9, 0.5), (0.9 - 0.45) / 0.55);
        let s = Siler { infant: 0.05, decay: 1.2, background: 0.004, old: 0.00004, slope: 0.095 };
        assert_eq!(s.survival(16.0), exp(-s.cumulative_hazard(16.0)));
    }
}
