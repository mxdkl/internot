//! Functions given by knots: piecewise-linear and log-linear interpolation,
//! piecewise-constant lookups, and first crossings.
//!
//! Knots are given by index (`x(i)`, `y(i)`), so callers keep their own
//! layouts (a `Vec<(i32, f64)>` of yearly anchors, a struct per anchor) and
//! nothing allocates. The arithmetic is fixed so results are the same on
//! every machine: with `f = (x − x_lo) / (x_hi − x_lo)`, linear is
//! `y_lo + f·(y_hi − y_lo)` and log-linear is
//! `exp((1 − f)·ln y_lo + f·ln y_hi)` through [`crate::dmath`]. Integer knots
//! (calendar years) are exact in `f64`, so passing them as `f64` changes
//! nothing.

use crate::dmath::{exp, ln};

/// Where `x` falls among increasing knots: between knots `lo` and `hi` at
/// fraction `f`, or clamped to an end (`lo == hi`, `f == 0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bracket {
    pub lo: usize,
    pub hi: usize,
    pub f: f64,
}

impl Bracket {
    /// Whether `x` lies at or beyond an end knot.
    #[inline]
    pub fn clamped(&self) -> bool {
        self.lo == self.hi
    }
}

/// The bracket of `x` among `n ≥ 1` increasing knots `knot(0..n)`: the first
/// knot at or above `x` and the one before it. At or below the first knot,
/// and above the last, the bracket is clamped to that knot.
#[inline]
pub fn bracket(n: usize, knot: impl Fn(usize) -> f64, x: f64) -> Bracket {
    assert!(n >= 1, "at least one knot");
    if x <= knot(0) {
        return Bracket {
            lo: 0,
            hi: 0,
            f: 0.0,
        };
    }
    for i in 1..n {
        let hi = knot(i);
        if x <= hi {
            let lo = knot(i - 1);
            return Bracket {
                lo: i - 1,
                hi: i,
                f: (x - lo) / (hi - lo),
            };
        }
    }
    Bracket {
        lo: n - 1,
        hi: n - 1,
        f: 0.0,
    }
}

/// `a + f·(b − a)`.
#[inline]
pub fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + f * (b - a)
}

/// `exp((1 − f)·ln a + f·ln b)`: geometric interpolation of positive values.
#[inline]
pub fn log_lerp(a: f64, b: f64, f: f64) -> f64 {
    exp((1.0 - f) * ln(a) + f * ln(b))
}

/// The piecewise-linear function through `n ≥ 1` knots `(x(i), y(i))` with
/// increasing `x`, constant outside them (the end values exactly).
#[inline]
pub fn piecewise_linear(
    n: usize,
    x: impl Fn(usize) -> f64,
    y: impl Fn(usize) -> f64,
    at: f64,
) -> f64 {
    let b = bracket(n, x, at);
    if b.clamped() {
        y(b.lo)
    } else {
        lerp(y(b.lo), y(b.hi), b.f)
    }
}

/// The piecewise log-linear (geometric) function through `n ≥ 1` knots with
/// increasing `x` and positive `y`, constant outside them.
#[inline]
pub fn piecewise_log_linear(
    n: usize,
    x: impl Fn(usize) -> f64,
    y: impl Fn(usize) -> f64,
    at: f64,
) -> f64 {
    let b = bracket(n, x, at);
    if b.clamped() {
        y(b.lo)
    } else {
        log_lerp(y(b.lo), y(b.hi), b.f)
    }
}

/// A piecewise-constant function by upper bounds: the value of the first
/// piece whose bound `x` is within (`x ≤ bound`), or `above` past them all.
#[inline]
pub fn step_at_most<T: PartialOrd>(
    pieces: impl IntoIterator<Item = (T, f64)>,
    x: T,
    above: f64,
) -> f64 {
    pieces.into_iter().find(|p| x <= p.0).map_or(above, |p| p.1)
}

/// A piecewise-constant function by exclusive upper bounds: the value of
/// the first piece whose bound `x` is below (`x < bound`), or `above` from
/// the last bound on.
#[inline]
pub fn step_below<T: PartialOrd>(
    pieces: impl IntoIterator<Item = (T, f64)>,
    x: T,
    above: f64,
) -> f64 {
    pieces.into_iter().find(|p| x < p.0).map_or(above, |p| p.1)
}

/// The value of the first inclusive interval `(lo, hi, value)` containing
/// `x`, or `outside`.
#[inline]
pub fn interval_value<T: PartialOrd>(
    intervals: impl IntoIterator<Item = (T, T, f64)>,
    x: T,
    outside: f64,
) -> f64 {
    intervals
        .into_iter()
        .find(|r| r.0 <= x && x <= r.1)
        .map_or(outside, |r| r.2)
}

/// The first integer in `lo..=hi` where `f` exceeds `v` (for a
/// nondecreasing `f` from 0 to 1 and a uniform `v`, an inverse-CDF draw).
#[inline]
pub fn first_above(lo: i32, hi: i32, f: impl Fn(i32) -> f64, v: f64) -> Option<i32> {
    (lo..=hi).find(|&x| f(x) > v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XS: [f64; 4] = [1900.0, 1950.0, 1960.0, 2000.0];
    const YS: [f64; 4] = [1.0, 3.0, 2.0, 2.5];

    fn pl(at: f64) -> f64 {
        piecewise_linear(4, |i| XS[i], |i| YS[i], at)
    }

    #[test]
    fn linear_hits_knots_interpolates_and_clamps() {
        assert_eq!(pl(1800.0), 1.0);
        assert_eq!(pl(1900.0), 1.0);
        assert_eq!(pl(1925.0), 2.0);
        assert_eq!(pl(1955.0), 2.5);
        assert_eq!(pl(2000.0), 2.5);
        assert_eq!(pl(2100.0), 2.5);
        for k in 0..4 {
            assert!((pl(XS[k]) - YS[k]).abs() < 1e-15);
        }
        // One knot: constant.
        assert_eq!(piecewise_linear(1, |_| 5.0, |_| 7.0, -3.0), 7.0);
        assert_eq!(piecewise_linear(1, |_| 5.0, |_| 7.0, 30.0), 7.0);
    }

    #[test]
    fn brackets_are_consistent_with_linear() {
        for at in [
            1850.0, 1900.0, 1901.5, 1950.0, 1959.0, 1999.0, 2000.0, 2001.0,
        ] {
            let b = bracket(4, |i| XS[i], at);
            let want = if b.clamped() {
                YS[b.lo]
            } else {
                lerp(YS[b.lo], YS[b.hi], b.f)
            };
            assert_eq!(pl(at).to_bits(), want.to_bits());
            assert!((0.0..=1.0).contains(&b.f));
        }
    }

    #[test]
    fn log_linear_is_geometric() {
        let y = [1.0, 4.0];
        let g = |at: f64| piecewise_log_linear(2, |i| [0.0, 2.0][i], |i| y[i], at);
        assert!((g(1.0) - 2.0).abs() < 1e-12);
        assert_eq!(g(-1.0), 1.0);
        assert_eq!(g(3.0), 4.0);
    }

    #[test]
    fn steps_and_intervals() {
        let steps = [(0, 0.0), (1, 0.15), (3, 0.14)];
        let s = |x| step_at_most(steps, x, 0.05);
        assert_eq!([s(-2), s(1), s(2), s(9)], [0.0, 0.15, 0.14, 0.05]);
        let bands = [(25.0, 1.0), (30.0, 0.8)];
        let b = |x| step_below(bands, x, 0.12);
        assert_eq!([b(24.9), b(25.0), b(29.0), b(30.0)], [1.0, 0.8, 0.8, 0.12]);
        let ranges = [(-1, -1, 0.09), (0, 0, 0.172), (2, 3, 0.1)];
        let r = |x| interval_value(ranges, x, 0.0);
        assert_eq!([r(-1), r(0), r(1), r(3)], [0.09, 0.172, 0.0, 0.1]);
        assert_eq!(
            first_above(2000, 2020, |y| (y - 2000) as f64 / 20.0, 0.5),
            Some(2011)
        );
        assert_eq!(first_above(2000, 2020, |_| 0.0, 0.5), None);
    }

    #[test]
    fn golden() {
        let bits = [pl(1933.0), pl(1957.3), pl(1987.0)].map(f64::to_bits);
        assert_eq!(bits, GOLDEN_LINEAR);
        let g = piecewise_log_linear(
            3,
            |i| [1900.0, 1950.0, 2000.0][i],
            |i| [3.0, 1.0, 0.25][i],
            1937.0,
        );
        assert_eq!(g.to_bits(), GOLDEN_LOG);
    }

    const GOLDEN_LINEAR: [u64; 3] = [
        4612406594367767184,
        4612294004377082931,
        4612446000864506675,
    ];
    const GOLDEN_LOG: u64 = 4608671364128153697;
}
