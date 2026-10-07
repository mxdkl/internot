//! Tabulated quantile functions, evaluated without transcendental calls.
//!
//! An [`OctaveTable`] stores a monotone function `f(w)` of a tail
//! probability `w ∈ (0, 1]` (for a quantile `u`, `w = 1 − u`) at knots that
//! are uniform within geometric octaves of `w`: octave `k` is
//! `[2^-(k+1), 2^-k)`, split into `2^{s_k}` equal cells
//! ([`cell_bits`]: 256 cells in the first octave, halving down to 16). So a
//! tail is resolved to any depth at a fixed cost per octave, and the body is
//! resolved finely.
//!
//! Evaluation reads the octave and cell straight from the bits of `w` (its
//! exponent and top mantissa bits), then interpolates linearly in `w`
//! between the cell's two knots. That is a table read and a multiply-add
//! instead of a logarithm and a search, and it is exactly monotone when the
//! knots are: within a cell the interpolation is monotone in `w`, and
//! adjacent cells share their boundary knot (each octave stores both ends).
//!
//! What it approximates: `f` itself at the knots (rounded to `f32`), and
//! between them the piecewise-linear interpolant in `w`, i.e. a law whose
//! density is constant on each cell. For a life table's death age given
//! survival to some age, 256 cells in the body put a knot every ~0.1–0.2
//! years of age, as fine as a quarter-year hazard table.
//!
//! [`OctaveTable::threshold`] inverts the table exactly: the least `w`
//! (as an `f64`) whose value passes a monotone predicate, by a search over
//! the knots and then over the float bits of `w` within one cell. With
//! [`count_from_threshold`] it gives exact counts of quantile slots
//! `w(ρ) = (2(n − ρ) − 1)/(2n)` on either side of the threshold.

/// Octaves a table may span: `w ≥ 2⁻⁶⁴` (smaller `w` read the deepest knot).
pub const MAX_OCTAVES: usize = 64;

/// Cell bits of octave `k`: `2^{s_k}` cells, 256 in the first octave
/// halving to 16 from the fifth on.
pub const fn cell_bits(k: usize) -> u32 {
    if k >= 4 { 4 } else { 8 - k as u32 }
}

/// `BASE[k]`: knots in octaves `0..k` (each `2^{s_j} + 1`, both ends).
const BASE: [u32; MAX_OCTAVES + 1] = {
    let mut b = [0u32; MAX_OCTAVES + 1];
    let mut k = 0;
    while k < MAX_OCTAVES {
        b[k + 1] = b[k] + (1 << cell_bits(k)) + 1;
        k += 1;
    }
    b
};

/// `2^-(52 − s_k)`: a cell's width in units of its octave's mantissa.
const FRAC: [f64; MAX_OCTAVES] = {
    let mut f = [0.0f64; MAX_OCTAVES];
    let mut k = 0;
    while k < MAX_OCTAVES {
        f[k] = f64::from_bits(((1023 - (52 - cell_bits(k))) as u64) << 52);
        k += 1;
    }
    f
};

const MANT: u64 = (1 << 52) - 1;

/// The knot layout of a table on octaves `k0..k1` (`w` from `2^-k1` to
/// `2^-k0`): tables of one shape can share one arena, each a run of
/// [`OctaveShape::len`] knots ([`OctaveTable`] owns its knots).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OctaveShape {
    k0: u8,
    k1: u8,
    /// `BASE[k1]`: octave `k` starts at `BASE[k1] − BASE[k + 1]`.
    top: u32,
}

impl OctaveShape {
    /// Octaves `k0..k1` (`k0 < k1 ≤ MAX_OCTAVES`). Panics on an empty or
    /// too deep range.
    pub fn new(k0: usize, k1: usize) -> Self {
        assert!(k0 < k1 && k1 <= MAX_OCTAVES, "octaves {k0}..{k1}");
        Self { k0: k0 as u8, k1: k1 as u8, top: BASE[k1] }
    }

    /// Knots per table.
    pub fn len(&self) -> usize {
        (BASE[self.k1 as usize] - BASE[self.k0 as usize]) as usize
    }

    /// Never empty.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Appends `f`'s knots to `out`, in increasing `w`.
    pub fn tabulate(&self, f: impl FnMut(f64) -> f64, out: &mut Vec<f32>) {
        let at = out.len();
        out.resize(at + self.len(), 0.0);
        self.tabulate_into(f, &mut out[at..]);
    }

    /// Writes `f`'s knots into `out` (exactly [`Self::len`] long).
    pub fn tabulate_into(&self, mut f: impl FnMut(f64) -> f64, out: &mut [f32]) {
        assert_eq!(out.len(), self.len(), "one slot per knot");
        let mut i = 0;
        for k in (self.k0 as usize..self.k1 as usize).rev() {
            let s = cell_bits(k);
            let low = f64::from_bits(((1023 - (k as i64 + 1)) as u64) << 52);
            for c in 0..=(1u64 << s) {
                // 2^-(k+1)·(1 + c/2^s), exact.
                out[i] = f(low * (1.0 + c as f64 / (1u64 << s) as f64)) as f32;
                i += 1;
            }
        }
    }

    /// The value at `w ≥ 0` of the table whose knots are `knots`
    /// (clamped outside `[2^-k1, 2^-k0]`).
    #[inline]
    pub fn eval(&self, knots: &[f32], w: f64) -> f64 {
        let bits = w.to_bits();
        // w ∈ [2^e, 2^(e+1)), octave k = −1 − e.
        let k = 1022 - ((bits >> 52) & 0x7ff) as i64;
        if k < self.k0 as i64 {
            return knots[self.len() - 1] as f64;
        }
        if k >= self.k1 as i64 {
            return knots[0] as f64;
        }
        let k = k as usize;
        let s = cell_bits(k);
        let m = bits & MANT;
        let c = (m >> (52 - s)) as usize;
        let frac = (m & ((1 << (52 - s)) - 1)) as f64 * FRAC[k];
        let i = (self.top - BASE[k + 1]) as usize + c;
        let (a, b) = (knots[i] as f64, knots[i + 1] as f64);
        a + frac * (b - a)
    }

    /// The `w` of knot `i`.
    fn knot_w(&self, i: usize) -> f64 {
        for k in (self.k0 as usize..self.k1 as usize).rev() {
            let start = (self.top - BASE[k + 1]) as usize;
            let len = (1usize << cell_bits(k)) + 1;
            if i < start + len {
                let low = f64::from_bits(((1023 - (k as i64 + 1)) as u64) << 52);
                return low * (1.0 + (i - start) as f64 / (1u64 << cell_bits(k)) as f64);
            }
        }
        unreachable!("knot {i} of {}", self.len())
    }

    /// The least `w ≥ 0` (an `f64`) whose value passes `pass`, for a
    /// predicate that, along increasing `w`, fails and then passes:
    /// `Some(0.0)` if every `w` passes, `None` if none does. Exact: every
    /// float below it fails, it and every float above pass.
    pub fn threshold(&self, knots: &[f32], pass: impl Fn(f64) -> bool) -> Option<f64> {
        let last = self.len() - 1;
        if !pass(knots[last] as f64) {
            return None;
        }
        if pass(knots[0] as f64) {
            return Some(0.0);
        }
        // First knot that passes: knots[i − 1] fails, knots[i] passes.
        let i = knots[..=last].partition_point(|&v| !pass(v as f64));
        let (mut a, mut b) = (self.knot_w(i - 1).to_bits(), self.knot_w(i).to_bits());
        while b - a > 1 {
            let mid = a + (b - a) / 2;
            if pass(self.eval(knots, f64::from_bits(mid))) { b = mid } else { a = mid }
        }
        Some(f64::from_bits(b))
    }

    /// [`Self::threshold`] given `v`, about the value where `pass` turns:
    /// the cell's interpolation is inverted at `v` and the exact float is
    /// found by galloping from there (a few evaluations instead of a
    /// bisection over the cell's floats). The same result for any `v`.
    pub fn threshold_near(&self, knots: &[f32], v: f64, pass: impl Fn(f64) -> bool) -> Option<f64> {
        let last = self.len() - 1;
        if !pass(knots[last] as f64) {
            return None;
        }
        if pass(knots[0] as f64) {
            return Some(0.0);
        }
        let i = knots[..=last].partition_point(|&v| !pass(v as f64));
        let (lo, hi) = (self.knot_w(i - 1), self.knot_w(i));
        let (a, b) = (knots[i - 1] as f64, knots[i] as f64);
        let f = if b != a { ((v - a) / (b - a)).clamp(0.0, 1.0) } else { 0.5 };
        let g = (lo + f * (hi - lo)).clamp(lo, hi);
        let (lo, hi, g) = (lo.to_bits(), hi.to_bits(), g.to_bits().clamp(lo.to_bits() + 1, hi.to_bits()));
        let ok = |x: u64| pass(self.eval(knots, f64::from_bits(x)));
        // Gallop to a bracket (fails at `a`, passes at `b`), then bisect.
        let (mut a, mut b);
        if ok(g) {
            b = g;
            let mut step = 1u64;
            loop {
                let x = b.saturating_sub(step).max(lo);
                if x == lo || !ok(x) {
                    a = x;
                    break;
                }
                b = x;
                step *= 2;
            }
        } else {
            a = g;
            let mut step = 1u64;
            loop {
                let x = (a + step).min(hi);
                if x == hi || ok(x) {
                    b = x;
                    break;
                }
                a = x;
                step *= 2;
            }
        }
        while b - a > 1 {
            let mid = a + (b - a) / 2;
            if ok(mid) { b = mid } else { a = mid }
        }
        Some(f64::from_bits(b))
    }
}

/// A monotone function of `w ∈ (0, 1]` on octaves `k0..k1` (`w` from
/// `2^-k1` to `2^-k0`), clamped outside them: an [`OctaveShape`] and its
/// knots.
#[derive(Clone, Debug)]
pub struct OctaveTable {
    knots: Vec<f32>,
    shape: OctaveShape,
}

impl OctaveTable {
    /// `f` tabulated on octaves `k0..k1` (`k0 < k1 ≤ MAX_OCTAVES`). Panics
    /// on an empty or too deep range.
    pub fn new(k0: usize, k1: usize, f: impl FnMut(f64) -> f64) -> Self {
        let shape = OctaveShape::new(k0, k1);
        let mut knots = Vec::with_capacity(shape.len());
        shape.tabulate(f, &mut knots);
        Self { knots, shape }
    }

    /// The octaves needed for quantile slots of `n` (`w ≥ 1/(2n)`), from
    /// octave `k0`.
    pub fn octaves_for(n: u64) -> usize {
        // 1/(2n) ≥ 2^-k1 ⟺ k1 ≥ log2(2n).
        ((64 - (2 * n.max(1) - 1).leading_zeros()) as usize).clamp(1, MAX_OCTAVES)
    }

    /// The table's value at `w ≥ 0` (clamped outside `[2^-k1, 2^-k0]`).
    #[inline]
    pub fn eval(&self, w: f64) -> f64 {
        self.shape.eval(&self.knots, w)
    }

    /// The value at quantile slot `rho` of `n`: `w = (2(n − ρ) − 1)/(2n)`
    /// (the slot's midpoint `u = (ρ + ½)/n`).
    #[inline]
    pub fn at_slot(&self, rho: u64, n: u64) -> f64 {
        self.eval(slot_w(rho, n))
    }

    /// Knots stored.
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Never empty (at least one octave).
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Heap bytes.
    pub fn heap_bytes(&self) -> usize {
        self.knots.capacity() * 4
    }

    #[cfg(test)]
    fn knot_w(&self, i: usize) -> f64 {
        self.shape.knot_w(i)
    }

    /// See [`OctaveShape::threshold`].
    pub fn threshold(&self, pass: impl Fn(f64) -> bool) -> Option<f64> {
        self.shape.threshold(&self.knots, pass)
    }

    /// See [`OctaveShape::threshold_near`].
    pub fn threshold_near(&self, v: f64, pass: impl Fn(f64) -> bool) -> Option<f64> {
        self.shape.threshold_near(&self.knots, v, pass)
    }
}

/// The slot `w` of `rho` of `n`: `(2(n − ρ) − 1)/(2n)`, correctly rounded,
/// so it falls as `ρ` rises.
#[inline]
pub fn slot_w(rho: u64, n: u64) -> f64 {
    debug_assert!(rho < n);
    (2 * (n - rho) - 1) as f64 / (2 * n) as f64
}

/// Slots `ρ ∈ [0, n)` with `slot_w(ρ, n) ≥ w` (a prefix, since `slot_w`
/// falls with `ρ`): an estimate corrected exactly.
pub fn count_from_threshold(n: u64, w: f64) -> u64 {
    if n == 0 {
        return 0;
    }
    // slot_w(ρ) ≥ w ⟺ 2(n − ρ) − 1 ≥ 2nw ⟺ ρ ≤ n − nw − ½.
    let est = (n as f64 - n as f64 * w + 0.5).floor().clamp(0.0, n as f64) as u64;
    let mut k = est;
    while k > 0 && slot_w(k - 1, n) < w {
        k -= 1;
    }
    while k < n && slot_w(k, n) >= w {
        k += 1;
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dmath::{ln, pow};

    /// A Gompertz-like death age given survival `w`: `16 + ln(1 − 50 ln w)`·10.
    fn age(w: f64) -> f64 {
        16.0 + 10.0 * ln(1.0 - 50.0 * ln(w))
    }

    #[test]
    fn exact_at_knots_and_close_between() {
        let n = 1u64 << 30;
        let t = OctaveTable::new(0, OctaveTable::octaves_for(n), age);
        // Knots: exact up to f32 rounding.
        for i in 0..t.len() {
            let w = t.knot_w(i);
            assert_eq!(t.eval(w) as f32, age(w) as f32, "knot {i} at {w}");
        }
        // Between knots: within 0.02 years everywhere, from the body to the
        // deepest slot.
        let mut worst = 0.0f64;
        for j in 0..200_000u64 {
            let rho = j * (n / 200_000);
            let w = slot_w(rho, n);
            worst = worst.max((t.eval(w) - age(w)).abs());
        }
        for rho in n - 1000..n {
            let w = slot_w(rho, n);
            worst = worst.max((t.eval(w) - age(w)).abs());
        }
        assert!(worst < 0.02, "worst {worst}");
    }

    #[test]
    fn monotone_in_w_across_cells_and_octaves() {
        let t = OctaveTable::new(0, 24, age);
        // Every 2^37th float down from 1 to below the table (cells and
        // octave boundaries included): the age never falls as w falls.
        let mut prev = f64::NEG_INFINITY;
        let mut w = 1.0f64;
        while w > 1e-9 {
            let v = t.eval(w);
            assert!(v >= prev, "{w}");
            prev = v;
            w = f64::from_bits(w.to_bits() - (1 << 37));
        }
        // Monotone over slots.
        let n = 1_000_003u64;
        let mut prev = f64::NEG_INFINITY;
        for rho in 0..n {
            let v = t.at_slot(rho, n);
            assert!(v >= prev, "slot {rho}");
            prev = v;
        }
    }

    #[test]
    fn clamps_outside_its_octaves() {
        let t = OctaveTable::new(1, 5, |w| w);
        assert_eq!(t.eval(0.75), 0.5);
        assert_eq!(t.eval(1.0), 0.5);
        assert_eq!(t.eval(2.0), 0.5);
        assert_eq!(t.eval(1e-9), 1.0 / 32.0);
        assert_eq!(t.eval(0.0), 1.0 / 32.0);
        assert!((t.eval(0.3) - 0.3).abs() < 1e-12);
    }

    #[test]
    fn threshold_and_counts_are_exact() {
        let n = 777_777u64;
        let t = OctaveTable::new(0, OctaveTable::octaves_for(n), age);
        for lim in [16.0, 16.3, 20.0, 45.5, 70.0, 79.25, 90.0, 101.0, 140.0, 0.0] {
            // Dead by `lim`: ages at most `lim` (passes at large w).
            let pass = |v: f64| v <= lim;
            let brute = (0..n).filter(|&r| pass(t.at_slot(r, n))).count() as u64;
            let got = t.threshold(pass).map_or(0, |w| count_from_threshold(n, w));
            assert_eq!(got, brute, "{lim}");
            // The galloping search agrees for any hint.
            for v in [lim, lim + 1e-9, lim - 3.0, 0.0, 1e9] {
                assert_eq!(t.threshold_near(v, pass), t.threshold(pass), "{lim} {v}");
            }
            if let Some(w) = t.threshold(pass) {
                if w > 0.0 {
                    assert!(pass(t.eval(w)));
                    assert!(!pass(t.eval(f64::from_bits(w.to_bits() - 1))));
                }
            }
        }
    }

    #[test]
    fn two_tailed_use_and_octave_counts() {
        // A log-logistic quantile through two one-sided tables meeting at ½.
        let g = |q: f64| pow(q / (1.0 - q), 1.0 / 6.0);
        let lo = OctaveTable::new(1, 40, g);
        let hi = OctaveTable::new(1, 40, |w| g(1.0 - w));
        let eval = |q: f64| if q < 0.5 { lo.eval(q) } else { hi.eval(1.0 - q) };
        let mut prev = 0.0;
        for j in 1..100_000 {
            let q = j as f64 / 100_000.0;
            let v = eval(q);
            assert!(v >= prev && (v - g(q)).abs() < 1e-4 * g(q).max(1.0), "{q}");
            prev = v;
        }
        assert_eq!(OctaveTable::octaves_for(1), 1);
        assert_eq!(OctaveTable::octaves_for(2), 2);
        assert_eq!(OctaveTable::octaves_for(4_000_000), 23);
        assert_eq!(lo.len(), (BASE[40] - BASE[1]) as usize);
    }

    #[test]
    fn golden() {
        let t = OctaveTable::new(0, 30, age);
        assert_eq!(t.len(), 257 + 129 + 65 + 33 + 26 * 17);
        assert_eq!(t.eval(0.5).to_bits(), 4632478512379330560);
        assert_eq!(t.eval(0.123456789).to_bits(), 4634006419898893213);
        assert_eq!(t.eval(3.0e-7).to_bits(), 4635486635140715881);
        assert_eq!(count_from_threshold(1000, 0.25), 750);
        assert_eq!(t.threshold(|v| v <= 70.0).unwrap().to_bits(), 4578173982875439586);
    }
}
