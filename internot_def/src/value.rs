//! The value vocabulary of world packs.
//!
//! | Type | RON | Meaning |
//! |---|---|---|
//! | [`Series`] | `[(1840, 0.004), (1960, 0.005)]` | piecewise-linear in year, clamped outside |
//! | [`VecSeries`] | `[(1840, [0.06, 0.05]), ...]` | the same, componentwise (distributions) |
//! | [`Steps`] | `(steps: [(34, 1.0), (44, 0.75)], above: 0.03)` | value by inclusive upper bound |
//! | [`Bands`] | `(below: [(25.0, 1.0), (30.0, 0.8)], above: 0.12)` | value by exclusive real upper bound |
//! | [`Ranges`] | `[(-1, 1, 0.09), (2, 3, 0.10)]` | value on inclusive integer ranges, 0 elsewhere |
//! | [`BySex`] | `(female: 0.9, male: 1.1)` | one value per sex |
//!
//! Every type checks itself with `validate`, which returns a message for
//! the caller to place (see [`crate::DefError::invalid`]).

use procedural_core::interp::{
    bracket, interval_value, lerp, piecewise_linear, step_at_most, step_below,
};
use serde::{Deserialize, Serialize};

/// A value by calendar year: piecewise-linear between `(year, value)`
/// anchors, constant before the first and after the last.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Series(pub Vec<(i32, f64)>);

impl Series {
    /// The value at `year` ([`piecewise_linear`]: `a + f·(b − a)` with
    /// `f = (year − y_a) / (y_b − y_a)` in `f64`, the same on every machine).
    pub fn at(&self, year: i32) -> f64 {
        let a = &self.0;
        piecewise_linear(a.len(), |i| a[i].0 as f64, |i| a[i].1, year as f64)
    }

    /// Anchors exist, years increase strictly, values are finite.
    pub fn validate(&self) -> Result<(), String> {
        validate_years(self.0.iter().map(|a| a.0))?;
        match self.0.iter().position(|a| !a.1.is_finite()) {
            Some(i) => Err(format!("anchor {i} is not a finite number")),
            None => Ok(()),
        }
    }

    /// [`Self::validate`], and every value in `[lo, hi]`.
    pub fn validate_within(&self, lo: f64, hi: f64) -> Result<(), String> {
        self.validate()?;
        match self.0.iter().position(|a| !(lo..=hi).contains(&a.1)) {
            Some(i) => Err(format!(
                "anchor {i} ({}) is outside [{lo}, {hi}]",
                self.0[i].1
            )),
            None => Ok(()),
        }
    }
}

/// Several values by calendar year, interpolated componentwise like
/// [`Series`]: for distributions and mixes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VecSeries(pub Vec<(i32, Vec<f64>)>);

impl VecSeries {
    /// Number of components.
    pub fn width(&self) -> usize {
        self.0[0].1.len()
    }

    /// The components at `year`, written into `out` (no allocation).
    /// Panics unless `out.len() == self.width()`.
    pub fn at_into(&self, year: i32, out: &mut [f64]) {
        let a = &self.0;
        assert_eq!(out.len(), self.width(), "component count");
        let b = bracket(a.len(), |i| a[i].0 as f64, year as f64);
        for (k, o) in out.iter_mut().enumerate() {
            *o = lerp(a[b.lo].1[k], a[b.hi].1[k], b.f);
        }
    }

    /// The components at `year` as a fixed array (panics on a width other
    /// than `N`; sections check widths when they load).
    pub fn at<const N: usize>(&self, year: i32) -> [f64; N] {
        let mut out = [0.0; N];
        self.at_into(year, &mut out);
        out
    }

    /// Anchors exist, years increase strictly, every anchor has `width`
    /// finite, non-negative components.
    pub fn validate(&self, width: usize) -> Result<(), String> {
        validate_years(self.0.iter().map(|a| a.0))?;
        for (i, (_, v)) in self.0.iter().enumerate() {
            if v.len() != width {
                return Err(format!(
                    "anchor {i} has {} values, expected {width}",
                    v.len()
                ));
            }
            if v.iter().any(|x| !x.is_finite() || *x < 0.0) {
                return Err(format!("anchor {i} has a negative or non-finite value"));
            }
        }
        Ok(())
    }

    /// [`Self::validate`], and every anchor sums to 1 within `1e-9`.
    pub fn validate_pmf(&self, width: usize) -> Result<(), String> {
        self.validate(width)?;
        for (i, (_, v)) in self.0.iter().enumerate() {
            let s: f64 = v.iter().sum();
            if (s - 1.0).abs() > 1e-9 {
                return Err(format!("anchor {i} sums to {s}, not 1"));
            }
        }
        Ok(())
    }
}

/// A value by an integer (an age, a duration): the first step whose
/// inclusive upper bound is at least `x`, else `above`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steps {
    /// `(upper bound, value)`, bounds increasing.
    pub steps: Vec<(i32, f64)>,
    /// The value past the last bound.
    pub above: f64,
}

impl Steps {
    /// The value at `x`.
    pub fn at(&self, x: i32) -> f64 {
        step_at_most(self.steps.iter().copied(), x, self.above)
    }

    /// Bounds increase strictly; values are finite.
    pub fn validate(&self) -> Result<(), String> {
        if self.steps.windows(2).any(|w| w[0].0 >= w[1].0) {
            return Err("step bounds must increase".into());
        }
        if self.steps.iter().any(|s| !s.1.is_finite()) || !self.above.is_finite() {
            return Err("a step value is not a finite number".into());
        }
        Ok(())
    }
}

/// A value by a real number (an age in years): the first band whose bound
/// `x` is below, else `above`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bands {
    /// `(exclusive upper bound, value)`, bounds increasing.
    pub below: Vec<(f64, f64)>,
    /// The value from the last bound on.
    pub above: f64,
}

impl Bands {
    /// The value at `x`.
    pub fn at(&self, x: f64) -> f64 {
        step_below(self.below.iter().copied(), x, self.above)
    }

    /// Bounds increase strictly; values are finite.
    pub fn validate(&self) -> Result<(), String> {
        if self.below.windows(2).any(|w| w[0].0 >= w[1].0) {
            return Err("band bounds must increase".into());
        }
        if self
            .below
            .iter()
            .any(|b| !b.0.is_finite() || !b.1.is_finite())
            || !self.above.is_finite()
        {
            return Err("a band is not a finite number".into());
        }
        Ok(())
    }
}

/// A value on inclusive integer ranges `(lo, hi, value)`, zero elsewhere.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ranges(pub Vec<(i32, i32, f64)>);

impl Ranges {
    /// The value at `x` (the first range containing it), or 0.
    pub fn at(&self, x: i32) -> f64 {
        interval_value(self.0.iter().copied(), x, 0.0)
    }

    /// Ranges are non-empty, disjoint and finite.
    pub fn validate(&self) -> Result<(), String> {
        for (i, r) in self.0.iter().enumerate() {
            if r.0 > r.1 {
                return Err(format!("range {i} ({}..={}) is empty", r.0, r.1));
            }
            if !r.2.is_finite() || r.2 < 0.0 {
                return Err(format!("range {i} has a negative or non-finite value"));
            }
            if self.0[..i].iter().any(|q| q.0 <= r.1 && r.0 <= q.1) {
                return Err(format!("range {i} ({}..={}) overlaps another", r.0, r.1));
            }
        }
        Ok(())
    }
}

/// One value per sex.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BySex<T> {
    pub female: T,
    pub male: T,
}

impl<T> BySex<T> {
    /// The female value if `female`, else the male one.
    pub fn get(&self, female: bool) -> &T {
        if female {
            &self.female
        } else {
            &self.male
        }
    }
}

fn validate_years(years: impl Iterator<Item = i32>) -> Result<(), String> {
    let years: Vec<i32> = years.collect();
    if years.is_empty() {
        return Err("needs at least one anchor".into());
    }
    match years.windows(2).position(|w| w[0] >= w[1]) {
        Some(i) => Err(format!(
            "anchor years must increase (anchor {} is {}, anchor {} is {})",
            i,
            years[i],
            i + 1,
            years[i + 1]
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_interpolates_and_clamps() {
        let s = Series(vec![(1900, 1.0), (2000, 3.0)]);
        assert_eq!(s.at(1800), 1.0);
        assert_eq!(s.at(1950), 2.0);
        assert_eq!(s.at(2100), 3.0);
        assert!(Series(vec![(2000, 1.0), (1900, 1.0)]).validate().is_err());
        assert!(Series(vec![]).validate().is_err());
    }

    #[test]
    fn vec_series_matches_componentwise_series() {
        let v = VecSeries(vec![(1900, vec![0.2, 0.8]), (1950, vec![0.5, 0.5])]);
        for year in [1850, 1900, 1925, 1937, 1950, 2000] {
            let [a, b] = v.at::<2>(year);
            let sa = Series(vec![(1900, 0.2), (1950, 0.5)]).at(year);
            let sb = Series(vec![(1900, 0.8), (1950, 0.5)]).at(year);
            assert_eq!((a.to_bits(), b.to_bits()), (sa.to_bits(), sb.to_bits()));
        }
        assert!(v.validate_pmf(2).is_ok());
        assert!(v.validate_pmf(3).is_err());
    }

    #[test]
    fn steps_and_ranges() {
        let s = Steps {
            steps: vec![(0, 0.0), (1, 0.15), (3, 0.14)],
            above: 0.05,
        };
        assert_eq!(
            [s.at(-2), s.at(1), s.at(2), s.at(9)],
            [0.0, 0.15, 0.14, 0.05]
        );
        let r = Ranges(vec![(-1, -1, 0.09), (0, 0, 0.172), (2, 3, 0.1)]);
        assert_eq!(
            [r.at(-1), r.at(0), r.at(1), r.at(3)],
            [0.09, 0.172, 0.0, 0.1]
        );
        assert!(Ranges(vec![(0, 2, 1.0), (2, 3, 1.0)]).validate().is_err());
    }

    #[test]
    fn ron_floats_parse_to_the_literals_bits() {
        let s: Series =
            ron::from_str("[(1840, 0.0110), (1855, 2.3e-5), (1900, 0.5121951219512195)]").unwrap();
        assert_eq!(s.0[0].1.to_bits(), 0.0110f64.to_bits());
        assert_eq!(s.0[1].1.to_bits(), 2.3e-5f64.to_bits());
        assert_eq!(s.0[2].1.to_bits(), (1.05f64 / 2.05).to_bits());
    }
}
