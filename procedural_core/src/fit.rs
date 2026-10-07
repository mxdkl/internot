//! Fitting tables and distributions to targets: iterative proportional
//! fitting, column raking, grouped fits, and exponential tilting.
//!
//! - [`ipf`]: iterative proportional fitting (Deming–Stephan 1940) of a dense
//!   nonnegative matrix to row and column margins, in place. The fixed point
//!   is `a_i · k_ij · b_j`: the matrix closest to `k` in KL divergence with
//!   those margins.
//! - [`GroupedIpf`]: the same fit when the kernel depends only on a class of
//!   each row and column (a birth year, a status). The fit runs over the
//!   classes, and each row and column takes its share of its class's margin:
//!   the fixed point has `a_i ∝ r_i` within a row class, so
//!   `x_ij = X[g_i][g_j] · r_i / R[g_i] · c_j / C[g_j]` exactly.
//! - [`rake_columns`]: IPF in factor form when every row's total is fixed by
//!   construction (each row is a distribution over the columns, scaled by its
//!   total). Only the column factors `b` are iterated; the row factors are
//!   closed-form normalizations. Used to split a list by a second margin
//!   (names by year split over groups so each group gets its share).
//! - [`tilt_mean`]: the exponential tilt `p_k θ^k / Z` of a distribution
//!   whose mean is a given multiple of the original's (the maximum-entropy
//!   change of a distribution to a new mean).
//!
//! Every result is a deterministic function of its inputs: loops have fixed
//! orders and fixed stopping rules, and transcendental math goes through
//! [`crate::dmath`].

use crate::dmath::exp;

/// When [`ipf`] stops: once every row sum is within `tol` (relative, against
/// `max(margin, 1)`) of its margin after a column pass, or after
/// `max_passes` column passes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IpfStop {
    pub tol: f64,
    pub max_passes: u32,
}

impl IpfStop {
    /// `1e-10` relative, at most 60 passes.
    pub const DEFAULT: IpfStop = IpfStop {
        tol: 1e-10,
        max_passes: 60,
    };
}

impl Default for IpfStop {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How an [`ipf`] run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpfOutcome {
    /// Column passes made.
    pub passes: u32,
    /// Whether the row sums met the tolerance (the column sums always do
    /// after a pass, for columns with positive mass).
    pub converged: bool,
}

/// Iterative proportional fitting of the row-major matrix `k` (`r.len()` rows
/// × `cols` columns) to row margins `r` and column margins `c`, in place.
///
/// Each pass scales rows to their margins, then columns to theirs. Rows or
/// columns with zero sum (or zero margin) become zero. The margins should
/// have equal totals; if they don't, the column margins win (the last scaling
/// is by columns) and the run ends unconverged.
///
/// Row-major throughout: column sums accumulate row by row, in the same order
/// as a column walk would, and each pass fuses a scaling with the next sums
/// ([`scale_cols_and_sum_rows`]), so results equal separate row and column
/// passes bit for bit.
///
/// Panics if `k.len() != r.len() * cols` or `c.len() != cols`.
pub fn ipf(k: &mut [f64], cols: usize, r: &[f64], c: &[f64], stop: IpfStop) -> IpfOutcome {
    let rows = r.len();
    assert_eq!(k.len(), rows * cols, "a row-major rows × cols matrix");
    assert_eq!(c.len(), cols, "one column margin per column");
    let mut colsum = vec![0.0; cols];
    let mut fac = vec![0.0; cols];
    let mut sums = vec![0.0; rows];
    if cols == 0 {
        return IpfOutcome {
            passes: 0,
            converged: r.iter().all(|&ri| ri.abs() <= stop.tol * ri.max(1.0)),
        };
    }
    scale_cols_and_sum_rows(k, cols, None, &mut sums);
    let mut passes = 0;
    loop {
        let converged = passes > 0
            && sums
                .iter()
                .zip(r)
                .all(|(&s, &ri)| (s - ri).abs() <= stop.tol * ri.max(1.0));
        if converged || passes == stop.max_passes {
            return IpfOutcome { passes, converged };
        }
        colsum.fill(0.0);
        for (row, (&s, &ri)) in k.chunks_exact_mut(cols).zip(sums.iter().zip(r)) {
            let f = if s > 0.0 { ri / s } else { 0.0 };
            for (x, acc) in row.iter_mut().zip(colsum.iter_mut()) {
                *x *= f;
                *acc += *x;
            }
        }
        for ((f, &s), &cj) in fac.iter_mut().zip(&colsum).zip(c) {
            *f = if s > 0.0 { cj / s } else { 0.0 };
        }
        scale_cols_and_sum_rows(k, cols, Some(&fac), &mut sums);
        passes += 1;
    }
}

/// Scale each column of a row-major `rows × cols` matrix by `fac` (if
/// given), then write each row's sum to `out`, added left to right exactly
/// as `iter().sum()` does. A float sum is a chain of dependent adds, so
/// eight rows are summed side by side to overlap their chains; each row's
/// own order, and so its result, is unchanged.
fn scale_cols_and_sum_rows(k: &mut [f64], cols: usize, fac: Option<&[f64]>, out: &mut [f64]) {
    const LANES: usize = 8;
    let mut i = 0;
    let mut chunks = out.chunks_exact_mut(LANES);
    for chunk in &mut chunks {
        let block = &mut k[i * cols..(i + LANES) * cols];
        let mut acc = [0.0f64; LANES];
        for j in 0..cols {
            let f = fac.map(|f| f[j]);
            for (l, a) in acc.iter_mut().enumerate() {
                let x = &mut block[l * cols + j];
                if let Some(f) = f {
                    *x *= f;
                }
                *a += *x;
            }
        }
        chunk.copy_from_slice(&acc);
        i += LANES;
    }
    for o in chunks.into_remainder() {
        let row = &mut k[i * cols..(i + 1) * cols];
        if let Some(fac) = fac {
            row.iter_mut().zip(fac).for_each(|(x, &f)| *x *= f);
        }
        *o = row.iter().sum();
        i += 1;
    }
}

/// Items grouped by a class key: the distinct keys in ascending order, each
/// item's class, and each class's summed margin (items added in order).
#[derive(Clone, Debug)]
pub struct Classes<K> {
    /// Distinct keys, ascending.
    pub keys: Vec<K>,
    /// `of[i]`: item `i`'s class (an index into `keys`).
    pub of: Vec<usize>,
    /// Each class's margin: the sum of its items' margins.
    pub sums: Vec<f64>,
}

impl<K: Ord + Copy> Classes<K> {
    /// Group items by `keys[i]`, summing `margins[i]` per class.
    pub fn new(keys: &[K], margins: &[f64]) -> Self {
        assert_eq!(keys.len(), margins.len(), "one margin per item");
        let mut distinct = keys.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        let of: Vec<usize> = keys
            .iter()
            .map(|k| distinct.binary_search(k).unwrap())
            .collect();
        let mut sums = vec![0.0; distinct.len()];
        for (&g, &v) in of.iter().zip(margins) {
            sums[g] += v;
        }
        Self {
            keys: distinct,
            of,
            sums,
        }
    }

    /// Item `i`'s share of its class's margin, given its margin `v` (zero if
    /// the class has none).
    #[inline]
    pub fn share(&self, i: usize, v: f64) -> f64 {
        share(v, self.sums[self.of[i]])
    }
}

#[inline]
fn share(v: f64, sum: f64) -> f64 {
    if sum > 0.0 {
        v / sum
    } else {
        0.0
    }
}

/// [`ipf`] for a kernel that depends only on the rows' and columns' classes.
///
/// The fit runs over the `classes × classes` kernel with the classes'
/// margins; then each item takes its share of its class's margin, so the
/// fitted entry for row `i` and column `j` is
/// `x[g_i][g_j] · row_share[i] · col_share[j]` ([`Self::entry`]). This is
/// exactly the full fit's fixed point, at `O(classes²)` per pass instead of
/// `O(rows · cols)`.
#[derive(Clone, Debug)]
pub struct GroupedIpf<KR, KC> {
    pub rows: Classes<KR>,
    pub cols: Classes<KC>,
    /// The fitted class matrix, row-major `rows.keys × cols.keys`.
    pub x: Vec<f64>,
    /// Each row's share of its class's margin.
    pub row_share: Vec<f64>,
    /// Each column's share of its class's margin.
    pub col_share: Vec<f64>,
    pub outcome: IpfOutcome,
}

impl<KR: Ord + Copy, KC: Ord + Copy> GroupedIpf<KR, KC> {
    /// Fit rows (class `row_keys[i]`, margin `r[i]`) and columns (class
    /// `col_keys[j]`, margin `c[j]`) under `kernel(row class, column class)`.
    pub fn new(
        row_keys: &[KR],
        r: &[f64],
        col_keys: &[KC],
        c: &[f64],
        kernel: impl Fn(&KR, &KC) -> f64,
        stop: IpfStop,
    ) -> Self {
        let rows = Classes::new(row_keys, r);
        let cols = Classes::new(col_keys, c);
        let (gr, gc) = (rows.keys.len(), cols.keys.len());
        let mut x = vec![0.0; gr * gc];
        for (a, kr) in rows.keys.iter().enumerate() {
            for (b, kc) in cols.keys.iter().enumerate() {
                x[a * gc + b] = kernel(kr, kc);
            }
        }
        let outcome = ipf(&mut x, gc, &rows.sums, &cols.sums, stop);
        let row_share = (0..r.len()).map(|i| rows.share(i, r[i])).collect();
        let col_share = (0..c.len()).map(|j| cols.share(j, c[j])).collect();
        Self {
            rows,
            cols,
            x,
            row_share,
            col_share,
            outcome,
        }
    }

    /// The fitted value for row `i` and column `j`.
    #[inline]
    pub fn entry(&self, i: usize, j: usize) -> f64 {
        let gc = self.cols.keys.len();
        self.x[self.rows.of[i] * gc + self.cols.of[j]] * self.row_share[i] * self.col_share[j]
    }

    /// Row class `a`'s cumulative weights over all columns,
    /// `W_a(j) = Σ_{j' ≤ j} x[a][g_j'] · col_share[j']`. Row `i` of class `a`
    /// has entries `row_share[i] · ΔW_a(j)`, so one cumulative row serves the
    /// whole class (see [`crate::partition::round_systematic_cumulative`]).
    pub fn class_cumulative(&self, a: usize) -> impl Iterator<Item = f64> + '_ {
        let gc = self.cols.keys.len();
        let row = &self.x[a * gc..(a + 1) * gc];
        let mut acc = 0.0;
        self.cols
            .of
            .iter()
            .zip(&self.col_share)
            .map(move |(&g, &s)| {
                acc += row[g] * s;
                acc
            })
    }

    /// The columns grouped by class, for [`Self::round_row`]: each class's
    /// columns in index order with their cumulative shares of the class's
    /// margin. `O(columns)`.
    pub fn columns_by_class(&self) -> ColumnsByClass {
        let gc = self.cols.keys.len();
        let mut start = vec![0usize; gc + 1];
        for &g in &self.cols.of {
            start[g + 1] += 1;
        }
        for g in 0..gc {
            start[g + 1] += start[g];
        }
        let mut next = start.clone();
        let mut cols = vec![0u32; self.cols.of.len()];
        for (j, &g) in self.cols.of.iter().enumerate() {
            cols[next[g]] = j as u32;
            next[g] += 1;
        }
        let mut cum = vec![0.0; cols.len()];
        for g in 0..gc {
            let mut acc = 0.0;
            for k in start[g]..start[g + 1] {
                acc += self.col_share[cols[k] as usize];
                cum[k] = acc;
            }
        }
        ColumnsByClass { start, cols, cum }
    }

    /// Row class `a`'s cumulative weights over the column classes,
    /// `C_a(g) = Σ_{g' ≤ g} x[a][g'] · s_g'`, where `s_g` is the sum of class
    /// `g`'s column shares (1, or 0 for a class without margin).
    pub fn class_weights_cumulative<'a>(
        &'a self,
        a: usize,
        by: &'a ColumnsByClass,
    ) -> impl Iterator<Item = f64> + 'a {
        let gc = self.cols.keys.len();
        let row = &self.x[a * gc..(a + 1) * gc];
        let mut acc = 0.0;
        (0..gc).map(move |g| {
            acc += row[g] * by.class_total(g);
            acc
        })
    }

    /// Keyed systematic rounding of row `i` (see
    /// [`crate::partition::round_systematic_cumulative`]) in two levels:
    /// points `u, u + 1, …` below the row's total fall first in a column
    /// class (along `class_cum`, row `i`'s class's
    /// [`Self::class_weights_cumulative`]), then in a column of that class
    /// (along the class's shares in `by`). This is exactly the one-level
    /// rounding along the columns ordered by class, at
    /// `O(points · log)` with no `classes × columns` table. `f(j, count)`
    /// receives the nonzero counts by class, then column index within a
    /// class (not in column order). Every entry gets the floor or the
    /// ceiling of its expectation, unbiased over `u`; the row's total is
    /// within one of its expectation.
    pub fn round_row(
        &self,
        i: usize,
        by: &ColumnsByClass,
        class_cum: &[f64],
        u: f64,
        mut f: impl FnMut(usize, u64),
    ) {
        let scale = self.row_share[i];
        let Some(&last) = class_cum.last() else {
            return;
        };
        let total = scale * last;
        if total.is_nan() || total <= 0.0 {
            return;
        }
        let gc = class_cum.len();
        let a = self.rows.of[i];
        let x = &self.x[a * gc..(a + 1) * gc];
        let (mut p, mut run) = (u, None::<(usize, u64)>);
        while p < total {
            let g = class_cum.partition_point(|&c| c * scale <= p).min(gc - 1);
            let below = if g == 0 {
                0.0
            } else {
                class_cum[g - 1] * scale
            };
            // The point's position within the class, in share units.
            let q = (p - below) / (x[g] * scale);
            let cum = &by.cum[by.start[g]..by.start[g + 1]];
            let k = cum.partition_point(|&c| c <= q).min(cum.len() - 1);
            let j = by.cols[by.start[g] + k] as usize;
            run = match run {
                Some((last, n)) if last == j => Some((last, n + 1)),
                Some((last, n)) => {
                    f(last, n);
                    Some((j, 1))
                }
                None => Some((j, 1)),
            };
            p += 1.0;
        }
        if let Some((j, n)) = run {
            f(j, n);
        }
    }
}

/// A [`GroupedIpf`]'s columns grouped by class
/// ([`GroupedIpf::columns_by_class`]).
#[derive(Clone, Debug)]
pub struct ColumnsByClass {
    /// Class `g`'s columns are `cols[start[g]..start[g + 1]]`.
    pub start: Vec<usize>,
    /// Column indices, by class, ascending within a class.
    pub cols: Vec<u32>,
    /// Cumulative column shares within each class, parallel to `cols`.
    pub cum: Vec<f64>,
}

impl ColumnsByClass {
    /// The sum of class `g`'s column shares.
    #[inline]
    pub fn class_total(&self, g: usize) -> f64 {
        if self.start[g + 1] > self.start[g] {
            self.cum[self.start[g + 1] - 1]
        } else {
            0.0
        }
    }
}

/// Column factors that rake a matrix with fixed row totals to column
/// targets: IPF in factor form.
///
/// Row `i` has total `row_total(i)` and column profile `q(i, ·)`; its weights
/// are `w_ih = row_total(i) · q(i, h) · b_h / Σ_g q(i, g) · b_g`
/// ([`raked_weight`]), so every row keeps its total whatever `b` is. Each pass
/// sets `b_h ← b_h · target_h / Σ_i w_ih` for columns with positive target
/// and positive current sum. Runs exactly `passes` passes (no early stop), so
/// the result is a fixed function of the inputs. Returns `b`.
pub fn rake_columns(
    rows: usize,
    cols: usize,
    row_total: impl Fn(usize) -> f64,
    q: impl Fn(usize, usize) -> f64,
    targets: &[f64],
    passes: usize,
) -> Vec<f64> {
    assert_eq!(targets.len(), cols, "one target per column");
    let mut b = vec![1.0; cols];
    for _ in 0..passes {
        let mut col = vec![0.0; cols];
        for i in 0..rows {
            let c = row_total(i);
            let z: f64 = (0..cols).map(|h| q(i, h) * b[h]).sum();
            if z > 0.0 {
                for (h, col) in col.iter_mut().enumerate() {
                    *col += c * q(i, h) * b[h] / z;
                }
            }
        }
        for h in 0..cols {
            if col[h] > 0.0 && targets[h] > 0.0 {
                b[h] *= targets[h] / col[h];
            }
        }
    }
    b
}

/// [`rake_columns`] over dense inputs (`q` row-major, `rows × cols`), stopping
/// after the first pass whose column sums are all within `tol` (relative)
/// of their targets, or after `max_passes`. Each pass is bit-identical to a
/// [`rake_columns`] pass, so the result equals `rake_columns` run for the
/// returned number of passes; the stop depends only on the inputs. Returns
/// `(b, passes)`.
pub fn rake_columns_dense(row_total: &[f64], q: &[f64], targets: &[f64], max_passes: usize, tol: f64) -> (Vec<f64>, usize) {
    let cols = targets.len();
    assert_eq!(q.len(), row_total.len() * cols, "one profile per row");
    let mut b = vec![1.0; cols];
    for pass in 1..=max_passes {
        let mut col = vec![0.0; cols];
        for (i, &c) in row_total.iter().enumerate() {
            let qi = &q[i * cols..(i + 1) * cols];
            let z: f64 = (0..cols).map(|h| qi[h] * b[h]).sum();
            if z > 0.0 {
                for (h, col) in col.iter_mut().enumerate() {
                    *col += c * qi[h] * b[h] / z;
                }
            }
        }
        let mut worst = 0.0f64;
        for h in 0..cols {
            if col[h] > 0.0 && targets[h] > 0.0 {
                worst = worst.max((targets[h] / col[h] - 1.0).abs());
                b[h] *= targets[h] / col[h];
            }
        }
        if worst <= tol {
            return (b, pass);
        }
    }
    (b, max_passes)
}

/// A raked row's weight in column `h`: `row_total · q(h) · b_h / Σ_g q(g) · b_g`,
/// or zero if the row has no mass under `b`. `q` is the row's profile.
#[inline]
pub fn raked_weight(row_total: f64, q: impl Fn(usize) -> f64, b: &[f64], h: usize) -> f64 {
    let z: f64 = (0..b.len()).map(|g| q(g) * b[g]).sum();
    if z > 0.0 {
        row_total * q(h) * b[h] / z
    } else {
        0.0
    }
}

/// Tilt the distribution `pmf` (over `0..len`) exponentially, `p_k θ^k`
/// normalized, so its mean becomes `factor` times the original's.
///
/// `θ` is found by 200 bisection steps on `ln θ ∈ [-20, 20]`, so the result
/// is the same on every machine; a target outside what that range can reach
/// saturates at the bound. A factor of exactly 1 leaves `pmf` unchanged.
/// The tilt is the maximum-entropy (minimum KL) change of the distribution
/// to the new mean.
pub fn tilt_mean(pmf: &mut [f64], factor: f64) {
    if factor == 1.0 {
        return;
    }
    let mean = |x: f64| {
        let (mut z, mut m) = (0.0, 0.0);
        for (k, &p) in pmf.iter().enumerate() {
            let w = p * exp(x * k as f64);
            z += w;
            m += w * k as f64;
        }
        m / z
    };
    let target = mean(0.0) * factor;
    let (mut lo, mut hi) = (-20.0f64, 20.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mean(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let x = 0.5 * (lo + hi);
    let mut z = 0.0;
    for (k, o) in pmf.iter_mut().enumerate() {
        *o *= exp(x * k as f64);
        z += *o;
    }
    pmf.iter_mut().for_each(|w| *w /= z);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    fn row_sums(k: &[f64], cols: usize) -> Vec<f64> {
        k.chunks_exact(cols).map(|r| r.iter().sum()).collect()
    }

    fn col_sums(k: &[f64], cols: usize) -> Vec<f64> {
        let mut s = vec![0.0; cols];
        for row in k.chunks_exact(cols) {
            for (a, x) in s.iter_mut().zip(row) {
                *a += x;
            }
        }
        s
    }

    /// The unfused fit: separate row and column passes, sums by `iter().sum()`.
    fn ipf_reference(k: &mut [f64], cols: usize, r: &[f64], c: &[f64], stop: IpfStop) {
        for it in 0..=stop.max_passes {
            let sums = row_sums(k, cols);
            let converged = it > 0
                && sums
                    .iter()
                    .zip(r)
                    .all(|(&s, &ri)| (s - ri).abs() <= stop.tol * ri.max(1.0));
            if converged || it == stop.max_passes {
                break;
            }
            for (row, (&s, &ri)) in k.chunks_exact_mut(cols).zip(sums.iter().zip(r)) {
                let f = if s > 0.0 { ri / s } else { 0.0 };
                row.iter_mut().for_each(|x| *x *= f);
            }
            let cs = col_sums(k, cols);
            for row in k.chunks_exact_mut(cols) {
                for ((x, &s), &cj) in row.iter_mut().zip(&cs).zip(c) {
                    *x *= if s > 0.0 { cj / s } else { 0.0 };
                }
            }
        }
    }

    fn random_problem(seed: u64, rows: usize, cols: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let key = Key::from_seed(seed);
        let k: Vec<f64> = (0..rows * cols)
            .map(|i| {
                let u = key.with2(1, i as u64).unit();
                if u < 0.15 {
                    0.0
                } else {
                    u * u * 3.0
                }
            })
            .collect();
        let mut r: Vec<f64> = (0..rows)
            .map(|i| 1.0 + 100.0 * key.with2(2, i as u64).unit())
            .collect();
        let c: Vec<f64> = (0..cols)
            .map(|j| 1.0 + 100.0 * key.with2(3, j as u64).unit())
            .collect();
        let (sr, sc): (f64, f64) = (r.iter().sum(), c.iter().sum());
        r.iter_mut().for_each(|x| *x *= sc / sr);
        (k, r, c)
    }

    #[test]
    fn ipf_matches_the_unfused_fit_bit_for_bit() {
        for seed in 0..40 {
            for (rows, cols) in [(1, 1), (3, 5), (8, 3), (9, 9), (17, 4), (33, 21)] {
                let (k, r, c) = random_problem(seed, rows, cols);
                let (mut a, mut b) = (k.clone(), k);
                ipf(&mut a, cols, &r, &c, IpfStop::DEFAULT);
                ipf_reference(&mut b, cols, &r, &c, IpfStop::DEFAULT);
                assert_eq!(
                    a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    b.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    "seed {seed}, {rows}×{cols}"
                );
            }
        }
    }

    #[test]
    fn ipf_meets_both_margins_on_a_positive_kernel() {
        for seed in 0..20 {
            let key = Key::from_seed(seed);
            let (rows, cols) = (12, 7);
            let mut k: Vec<f64> = (0..rows * cols)
                .map(|i| 0.1 + key.with(i as u64).unit())
                .collect();
            let (_, r, c) = random_problem(seed, rows, cols);
            let out = ipf(&mut k, cols, &r, &c, IpfStop::DEFAULT);
            assert!(out.converged, "{out:?}");
            for (s, m) in row_sums(&k, cols).iter().zip(&r) {
                assert!((s - m).abs() <= 1e-9 * m.max(1.0));
            }
            for (s, m) in col_sums(&k, cols).iter().zip(&c) {
                assert!((s - m).abs() <= 1e-9 * m.max(1.0));
            }
        }
    }

    #[test]
    fn ipf_keeps_the_cross_ratios() {
        // The fit is a_i k_ij b_j: odds ratios of the kernel survive.
        let (rows, cols) = (4, 5);
        let k0: Vec<f64> = (0..rows * cols).map(|i| 1.0 + (i % 7) as f64).collect();
        let r = [10.0, 20.0, 30.0, 40.0];
        let c = [5.0, 15.0, 25.0, 35.0, 20.0];
        let mut k = k0.clone();
        ipf(&mut k, cols, &r, &c, IpfStop::DEFAULT);
        let at = |m: &[f64], i: usize, j: usize| m[i * cols + j];
        for (i, j) in [(0, 0), (1, 2), (3, 4)] {
            let ratio = |m: &[f64]| at(m, i, j) * at(m, 2, 1) / (at(m, i, 1) * at(m, 2, j));
            assert!((ratio(&k) - ratio(&k0)).abs() < 1e-9 * ratio(&k0));
        }
    }

    #[test]
    fn ipf_zero_margins_and_zero_columns_stay_zero() {
        let mut k = vec![1.0, 0.0, 2.0, 3.0, 0.0, 1.0];
        let out = ipf(&mut k, 3, &[0.0, 4.0], &[1.0, 0.0, 3.0], IpfStop::DEFAULT);
        assert!(out.converged);
        assert_eq!(&k[..3], &[0.0, 0.0, 0.0]);
        assert_eq!(k[4], 0.0);
        assert!((k[3] + k[5] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn ipf_golden() {
        // Pinned: the ledger's markets depend on these exact values.
        let (mut k, r, c) = random_problem(7, 5, 4);
        let out = ipf(&mut k, 4, &r, &c, IpfStop::DEFAULT);
        let h = k.iter().fold(0u64, |h, x| h.rotate_left(7) ^ x.to_bits());
        assert_eq!((out.passes, out.converged, h), GOLDEN_IPF);
    }

    const GOLDEN_IPF: (u32, bool, u64) = (27, true, 5303345616636775852);

    #[test]
    fn grouped_fit_equals_the_full_fit() {
        // A kernel by class gives the same entries as the full fit.
        let row_keys = [3, 1, 3, 2, 1, 1, 2];
        let col_keys = [0u8, 1, 0, 0, 1];
        let r = [5.0, 2.0, 1.0, 7.0, 3.0, 0.5, 2.5];
        let mut c = [4.0, 6.0, 1.0, 3.0, 7.0];
        let (sr, sc): (f64, f64) = (r.iter().sum(), c.iter().sum());
        c.iter_mut().for_each(|x| *x *= sr / sc);
        let kernel = |a: &i32, b: &u8| 1.0 + (*a as f64 - 2.0 * *b as f64).abs();
        let g = GroupedIpf::new(&row_keys, &r, &col_keys, &c, kernel, IpfStop::DEFAULT);
        assert_eq!(g.rows.keys, vec![1, 2, 3]);
        assert_eq!(g.cols.keys, vec![0, 1]);
        let mut full: Vec<f64> = row_keys
            .iter()
            .flat_map(|a| col_keys.iter().map(move |b| kernel(a, b)))
            .collect();
        let stop = IpfStop {
            tol: 1e-14,
            max_passes: 2000,
        };
        ipf(&mut full, c.len(), &r, &c, stop);
        for i in 0..r.len() {
            for j in 0..c.len() {
                let want = full[i * c.len() + j];
                assert!(
                    (g.entry(i, j) - want).abs() < 1e-8 * want.max(1.0),
                    "{i},{j}"
                );
            }
            // Class cumulative weights give the row's entries.
            let cum: Vec<f64> = g.class_cumulative(g.rows.of[i]).collect();
            let mut prev = 0.0;
            for (j, &w) in cum.iter().enumerate() {
                let e = g.row_share[i] * (w - prev);
                assert!((e - g.entry(i, j)).abs() < 1e-9, "{i},{j}");
                prev = w;
            }
        }
    }

    #[test]
    fn two_level_rounding_is_unbiased_and_matches_entries() {
        let row_keys = [3, 1, 3, 2, 1, 1, 2];
        let col_keys = [0u8, 1, 0, 2, 1, 0, 2, 1];
        let r = [5.0, 2.0, 1.0, 7.0, 3.0, 0.5, 2.5];
        let mut c = [4.0, 6.0, 1.0, 3.0, 7.0, 0.25, 2.0, 1.5];
        let (sr, sc): (f64, f64) = (r.iter().sum(), c.iter().sum());
        c.iter_mut().for_each(|x| *x *= sr / sc);
        let kernel = |a: &i32, b: &u8| 1.0 + (*a as f64 - 2.0 * *b as f64).abs();
        let g = GroupedIpf::new(&row_keys, &r, &col_keys, &c, kernel, IpfStop::DEFAULT);
        let by = g.columns_by_class();
        assert_eq!(by.cols, vec![0, 2, 5, 1, 4, 7, 3, 6]);
        let grid = 1 << 11;
        for i in 0..r.len() {
            let a = g.rows.of[i];
            let class_cum: Vec<f64> = g.class_weights_cumulative(a, &by).collect();
            let expect: Vec<f64> = (0..c.len()).map(|j| g.entry(i, j)).collect();
            let total: f64 = expect.iter().sum();
            let mut mean = vec![0.0; c.len()];
            for k in 0..grid {
                let u = (k as f64 + 0.5) / grid as f64;
                let mut got = vec![0u64; c.len()];
                g.round_row(i, &by, &class_cum, u, |j, n| got[j] += n);
                let sum: u64 = got.iter().sum();
                assert!((sum as f64 - total).abs() < 1.0 + 1e-9, "row {i} total");
                for j in 0..c.len() {
                    let e = expect[j];
                    assert!(
                        got[j] as f64 >= e.floor() - 1e-9 && got[j] as f64 <= e.ceil() + 1e-9,
                        "row {i} col {j}: {} for {e}",
                        got[j]
                    );
                    mean[j] += got[j] as f64 / grid as f64;
                }
            }
            for j in 0..c.len() {
                assert!(
                    (mean[j] - expect[j]).abs() < 2.0 / grid as f64 + 1e-9,
                    "row {i} col {j} mean"
                );
            }
        }
    }

    #[test]
    fn two_level_rounding_golden() {
        let row_keys = [0u8, 1, 1, 2];
        let col_keys = [1u8, 0, 1, 2, 0];
        let r = [3.5, 1.25, 4.0, 2.25];
        let c = [2.0, 3.0, 1.0, 1.5, 3.5];
        let kernel = |a: &u8, b: &u8| 1.0 / (1.0 + (*a as f64 - *b as f64).abs());
        let g = GroupedIpf::new(&row_keys, &r, &col_keys, &c, kernel, IpfStop::DEFAULT);
        let by = g.columns_by_class();
        let mut got = Vec::new();
        for i in 0..r.len() {
            let class_cum: Vec<f64> = g.class_weights_cumulative(g.rows.of[i], &by).collect();
            g.round_row(
                i,
                &by,
                &class_cum,
                Key::from_seed(i as u64).unit(),
                |j, n| got.push((i, j, n)),
            );
        }
        assert_eq!(got, GOLDEN_TWO_LEVEL);
    }

    const GOLDEN_TWO_LEVEL: [(usize, usize, u64); 11] = [
        (0, 1, 1),
        (0, 4, 2),
        (0, 3, 1),
        (1, 1, 1),
        (1, 3, 1),
        (2, 1, 1),
        (2, 4, 1),
        (2, 0, 1),
        (2, 3, 1),
        (3, 4, 1),
        (3, 3, 1),
    ];

    #[test]
    fn raking_keeps_rows_and_meets_column_targets() {
        let (rows, cols) = (40, 4);
        let key = Key::from_seed(3);
        let total = |i: usize| 1.0 + (i % 9) as f64;
        let q = |i: usize, h: usize| key.with2(i as u64, h as u64).unit() + 0.01;
        let sum: f64 = (0..rows).map(total).sum();
        let targets: Vec<f64> = [0.4, 0.3, 0.2, 0.1].iter().map(|p| p * sum).collect();
        let b = rake_columns(rows, cols, total, q, &targets, 200);
        let mut col = vec![0.0; cols];
        for i in 0..rows {
            let w: Vec<f64> = (0..cols)
                .map(|h| raked_weight(total(i), |g| q(i, g), &b, h))
                .collect();
            assert!((w.iter().sum::<f64>() - total(i)).abs() < 1e-9);
            col.iter_mut().zip(&w).for_each(|(c, x)| *c += x);
        }
        for (c, t) in col.iter().zip(&targets) {
            assert!((c - t).abs() < 1e-6 * t, "{c} vs {t}");
        }
    }

    #[test]
    fn raking_golden() {
        let key = Key::from_seed(11);
        let b = rake_columns(
            25,
            3,
            |i| (i + 1) as f64,
            |i, h| key.with2(i as u64, h as u64).unit(),
            &[100.0, 125.0, 100.0],
            60,
        );
        let bits: Vec<u64> = b.iter().map(|x| x.to_bits()).collect();
        assert_eq!(bits, GOLDEN_RAKE);
    }

    const GOLDEN_RAKE: [u64; 3] = [
        4604476580790536433,
        4609168870358131374,
        4607213674938408017,
    ];

    fn mean(p: &[f64]) -> f64 {
        p.iter().enumerate().map(|(k, &x)| k as f64 * x).sum()
    }

    #[test]
    fn tilting_scales_the_mean_and_keeps_a_distribution() {
        let base = [0.05, 0.1, 0.3, 0.25, 0.15, 0.1, 0.05];
        for factor in [0.5, 0.8, 1.0, 1.1, 1.6] {
            let mut p = base;
            tilt_mean(&mut p, factor);
            assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            assert!((mean(&p) - factor * mean(&base)).abs() < 1e-9, "{factor}");
            // Ratios p_k / base_k are geometric in k.
            let r: Vec<f64> = p.iter().zip(&base).map(|(a, b)| a / b).collect();
            for k in 2..r.len() {
                assert!((r[k] / r[k - 1] - r[1] / r[0]).abs() < 1e-9);
            }
        }
        let mut same = base;
        tilt_mean(&mut same, 1.0);
        assert_eq!(same, base);
    }

    #[test]
    fn tilt_golden() {
        let mut p = [0.2, 0.3, 0.25, 0.15, 0.1];
        tilt_mean(&mut p, 1.3);
        let bits: Vec<u64> = p.iter().map(|x| x.to_bits()).collect();
        assert_eq!(bits, GOLDEN_TILT);
    }

    const GOLDEN_TILT: [u64; 5] = [
        4592680935637185271,
        4597361702315300921,
        4598324266869614620,
        4596776774931541614,
        4596080829253300131,
    ];

    #[test]
    fn dense_rake_equals_rake_columns_for_its_passes() {
        let key = crate::key::Key::from_seed(17);
        let (rows, cols) = (300, 5);
        let total: Vec<f64> = (0..rows).map(|i| 1.0 + key.with2(i as u64, 0).below(1000) as f64).collect();
        let q: Vec<f64> = (0..rows * cols).map(|k| key.with2(k as u64, 1).unit()).collect();
        let targets = [0.4, 0.3, 0.15, 0.1, 0.05].map(|t| t * total.iter().sum::<f64>());
        let (b, passes) = rake_columns_dense(&total, &q, &targets, 500, 1e-9);
        assert!(passes > 1 && passes < 500, "{passes}");
        let r = rake_columns(rows, cols, |i| total[i], |i, h| q[i * cols + h], &targets, passes);
        assert_eq!(b.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), r.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        // The columns meet their targets.
        for h in 0..cols {
            let got: f64 = (0..rows).map(|i| raked_weight(total[i], |g| q[i * cols + g], &b, h)).sum();
            assert!((got / targets[h] - 1.0).abs() < 1e-8, "{h}: {got} vs {}", targets[h]);
        }
    }
}
