//! Partitions of `[0, n)` into groups with controlled sizes and no holes.
//!
//! Compose a partition with a keyed permutation to get random groups:
//! `group_of(perm.fwd(x))` places `x`, and `range_of(g).map(|i| perm.inv(i))`
//! lists a group's members. Both directions are pure functions.
//!
//! - [`SizeClasses`]: groups laid out by size from an exact histogram. With a
//!   uniform permutation this is a uniformly random set partition with that
//!   exact histogram (the symmetric group acts transitively on partitions of
//!   one type). `group_of` is `O(log K)` for `K` size classes.
//! - [`SplitTree`]: recursive random splits (fragmentation). Leaf sizes are
//!   products of split fractions, so they are roughly lognormal; nesting
//!   (region → neighbourhood → block) comes free.
//!
//! - [`apportion_systematic`] and [`contingency_systematic`]: keyed, unbiased
//!   integer splits with exact totals (one margin, or two), for counts that
//!   both sides of a relation must agree on. [`SystematicShares`] precomputes
//!   the shares for many splits over the same weights.
//!
//! See `docs/superpowers/research/2026-09-29-local-access-and-bijections.md` §4.

use std::ops::Range;

use crate::key::Key;

/// A partition of `[0, len)` into numbered groups.
pub trait Partition {
    /// Size of the partitioned range.
    fn len(&self) -> u64;
    /// Number of groups.
    fn num_groups(&self) -> u64;
    /// `(group, slot)` containing index `i`. Panics if `i >= len`.
    fn group_of(&self, i: u64) -> (u64, u64);
    /// The index range of group `g`. Panics if `g >= num_groups`.
    fn range_of(&self, g: u64) -> Range<u64>;
    /// True if the range is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, Debug)]
struct SizeClass {
    size: u64,
    count: u64,
    first_index: u64,
    first_group: u64,
}

/// Groups laid out by size from an exact `(size, count)` histogram.
#[derive(Clone, Debug)]
pub struct SizeClasses {
    classes: Vec<SizeClass>,
    len: u64,
    groups: u64,
}

impl SizeClasses {
    /// Build from `(size, count)` pairs. Sizes must be positive; pairs with
    /// the same size are merged.
    pub fn from_histogram(histogram: &[(u64, u64)]) -> Self {
        let mut hist: Vec<(u64, u64)> = histogram.iter().copied().filter(|&(_, c)| c > 0).collect();
        assert!(
            hist.iter().all(|&(s, _)| s > 0),
            "group sizes must be positive"
        );
        hist.sort_unstable();
        let mut classes: Vec<SizeClass> = Vec::with_capacity(hist.len());
        let (mut index, mut group) = (0u64, 0u64);
        for (size, count) in hist {
            if let Some(last) = classes.last_mut() {
                if last.size == size {
                    last.count += count;
                    index += size * count;
                    group += count;
                    continue;
                }
            }
            classes.push(SizeClass {
                size,
                count,
                first_index: index,
                first_group: group,
            });
            index += size * count;
            group += count;
        }
        Self {
            classes,
            len: index,
            groups: group,
        }
    }

    /// The `(size, count)` histogram this partition realises.
    pub fn histogram(&self) -> Vec<(u64, u64)> {
        self.classes.iter().map(|c| (c.size, c.count)).collect()
    }
}

impl Partition for SizeClasses {
    fn len(&self) -> u64 {
        self.len
    }

    fn num_groups(&self) -> u64 {
        self.groups
    }

    #[inline]
    fn group_of(&self, i: u64) -> (u64, u64) {
        assert!(i < self.len, "SizeClasses::group_of: {i} out of range");
        let ci = self.classes.partition_point(|c| c.first_index <= i) - 1;
        let c = &self.classes[ci];
        let off = i - c.first_index;
        (c.first_group + off / c.size, off % c.size)
    }

    #[inline]
    fn range_of(&self, g: u64) -> Range<u64> {
        assert!(g < self.groups, "SizeClasses::range_of: {g} out of range");
        let ci = self.classes.partition_point(|c| c.first_group <= g) - 1;
        let c = &self.classes[ci];
        let start = c.first_index + (g - c.first_group) * c.size;
        start..start + c.size
    }
}

/// Turn a group-size pmf into an exact histogram covering exactly `n`
/// members.
///
/// `pmf` gives the probability of a *group* having each size; it is not the
/// size-biased distribution a random member sees. Counts are the
/// largest-remainder rounding of `G · p_s` with `G = n / E[size]`, and any
/// remainder is absorbed by singletons, so `Σ size · count == n` always.
pub fn histogram_from_pmf(n: u64, pmf: &[(u64, f64)]) -> Vec<(u64, u64)> {
    assert!(pmf.iter().all(|&(s, p)| s > 0 && p >= 0.0), "invalid pmf");
    let total_p: f64 = pmf.iter().map(|&(_, p)| p).sum();
    assert!(total_p > 0.0, "pmf has no mass");
    let mean: f64 = pmf.iter().map(|&(s, p)| s as f64 * p).sum::<f64>() / total_p;
    let groups = n as f64 / mean;
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(pmf.len() + 1);
    let mut rema: Vec<(f64, usize)> = Vec::with_capacity(pmf.len());
    let mut used = 0u64;
    for (idx, &(s, p)) in pmf.iter().enumerate() {
        let exact = groups * p / total_p;
        let mut c = exact.floor() as u64;
        // Never overshoot n.
        c = c.min((n - used) / s);
        used += c * s;
        out.push((s, c));
        rema.push((exact - exact.floor(), idx));
    }
    // Largest remainder first, while it still fits.
    rema.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
    for (_, idx) in rema {
        let s = out[idx].0;
        if used + s <= n {
            out[idx].1 += 1;
            used += s;
        }
    }
    if used < n {
        out.push((1, n - used));
    }
    out
}

/// Fixed-point scale of [`apportion_systematic`]'s cumulative shares.
const SHARE_ONE: u128 = 1 << 40;

/// Keyed systematic apportionment: split `n` into integer parts
/// proportional to `weights`, written to `out` (same length).
///
/// With cumulative shares `F_c` and one keyed uniform `u`, part `c` is
/// `⌊n·F_c + u⌋ − ⌊n·F_{c−1} + u⌋`. The parts sum to `n` exactly; each is
/// within 1 of `n·p_c`, and its mean over keys is `n·p_c`. Largest
/// remainder, by contrast, gives a one-member split to the modal class
/// every time, so small splits never reach the rarer classes. Shares are
/// fixed-point (2⁻⁴⁰) and the rounding is integer arithmetic, so the result
/// is exact and portable. Non-positive (and NaN) weights get nothing.
///
/// Panics if `n > 0` and no weight is positive, or if the lengths differ.
pub fn apportion_systematic(n: u64, weights: &[f64], key: Key, out: &mut [u64]) {
    assert_eq!(weights.len(), out.len(), "one part per weight");
    out.fill(0);
    if n == 0 {
        return;
    }
    let u = key.below(SHARE_ONE as u64) as u128;
    let mut prev = 0u128;
    cumulative_shares(weights, |i, share| {
        let cur = (n as u128 * share + u) / SHARE_ONE;
        out[i] = (cur - prev) as u64;
        prev = cur;
    });
}

/// The fixed-point cumulative share at the end of each part, in order:
/// nondecreasing, and exactly [`SHARE_ONE`] from the last positive weight
/// on. Panics if no weight is positive.
fn cumulative_shares(weights: &[f64], mut f: impl FnMut(usize, u128)) {
    let w = |x: f64| if x > 0.0 { x } else { 0.0 };
    let total: f64 = weights.iter().map(|&x| w(x)).sum();
    assert!(total > 0.0 && total.is_finite(), "no positive weight");
    let last = weights.iter().rposition(|&x| x > 0.0).unwrap();
    let (mut acc, mut prev_share) = (0.0f64, 0u128);
    for (i, &x) in weights.iter().enumerate() {
        acc += w(x);
        let share = if i >= last {
            SHARE_ONE
        } else {
            ((acc / total * SHARE_ONE as f64) as u128).clamp(prev_share, SHARE_ONE)
        };
        f(i, share);
        prev_share = share;
    }
}

/// [`apportion_systematic`] with the cumulative shares computed once, for
/// many splits over the same weights. Gives exactly the same parts, densely
/// ([`Self::apportion`]) or as the nonzero parts only
/// ([`Self::for_each_part`], `O(parts · log len)`, for small `n`).
#[derive(Clone, Debug)]
pub struct SystematicShares {
    /// Cumulative fixed-point share at the end of each part.
    cum: Vec<u64>,
}

impl SystematicShares {
    /// Shares of `weights`. Panics if no weight is positive.
    pub fn new(weights: &[f64]) -> Self {
        let mut cum = Vec::with_capacity(weights.len());
        cumulative_shares(weights, |_, share| cum.push(share as u64));
        Self { cum }
    }

    /// Number of parts.
    pub fn len(&self) -> usize {
        self.cum.len()
    }

    /// Whether there are no parts (never: some weight is positive).
    pub fn is_empty(&self) -> bool {
        self.cum.is_empty()
    }

    /// The parts of `n`, as [`apportion_systematic`] gives them.
    pub fn apportion(&self, n: u64, key: Key, out: &mut [u64]) {
        assert_eq!(self.cum.len(), out.len(), "one part per weight");
        out.fill(0);
        if n == 0 {
            return;
        }
        let u = key.below(SHARE_ONE as u64) as u128;
        let mut prev = 0u128;
        for (&share, o) in self.cum.iter().zip(out.iter_mut()) {
            let cur = (n as u128 * share as u128 + u) / SHARE_ONE;
            *o = (cur - prev) as u64;
            prev = cur;
        }
    }

    /// Calls `f(part, count)` for each nonzero part of `n`, in part order:
    /// the same parts as [`Self::apportion`]. Point `k` (`1..=n`) of the
    /// systematic sample lands in the first part whose `⌊n·F + u⌋` reaches
    /// `k`, so each run of points costs one binary search.
    pub fn for_each_part(&self, n: u64, key: Key, mut f: impl FnMut(usize, u64)) {
        if n == 0 {
            return;
        }
        let u = key.below(SHARE_ONE as u64) as u128;
        let n128 = n as u128;
        let (mut k, mut lo) = (1u64, 0usize);
        while k <= n {
            let target = k as u128 * SHARE_ONE;
            let i = lo + self.cum[lo..].partition_point(|&c| n128 * c as u128 + u < target);
            let cur = ((n128 * self.cum[i] as u128 + u) / SHARE_ONE) as u64;
            f(i, cur - (k - 1));
            k = cur + 1;
            lo = i + 1;
        }
    }
}

/// An integer table with exact margins: row `r` holds `rows[r]` in all,
/// column `c` holds `cols[c]`. Rows are filled in order, each apportioned
/// systematically (as in [`apportion_systematic`]) over what the columns
/// have left: the hypergeometric mean, so each entry's mean over keys is
/// `rows[r]·cols[c]/N`, and the last row takes exactly the remainder. `out` is
/// row-major, `rows.len() × cols.len()`. Allocates one scratch vector; meant
/// for build time.
///
/// Panics if the margins' totals differ or `out` has the wrong length.
pub fn contingency_systematic(rows: &[u64], cols: &[u64], key: Key, out: &mut [u64]) {
    let (nr, nc) = (rows.len(), cols.len());
    assert_eq!(out.len(), nr * nc, "a row-major rows × cols table");
    assert_eq!(
        rows.iter().sum::<u64>(),
        cols.iter().sum::<u64>(),
        "margins must have the same total"
    );
    let mut left = cols.to_vec();
    let mut remaining: u64 = cols.iter().sum();
    for (r, &m) in rows.iter().enumerate() {
        let row = &mut out[r * nc..(r + 1) * nc];
        row.fill(0);
        if m == 0 {
            continue;
        }
        let total = remaining as u128;
        let u = key.with(r as u64).below(remaining) as u128;
        let (mut cum, mut prev) = (0u128, 0u128);
        for (cell, l) in row.iter_mut().zip(left.iter_mut()) {
            cum += *l as u128;
            let cur = (m as u128 * cum + u) / total;
            *cell = (cur - prev) as u64;
            *l -= *cell;
            prev = cur;
        }
        remaining -= m;
    }
}

/// Recursive random splits of `[0, n)` (fragmentation partition).
///
/// A node `[lo, hi)` becomes a leaf when its size is at most a size drawn
/// uniformly from `[min_leaf, max_leaf]` for that node. Otherwise it splits at
/// a triangular-distributed fraction in `[0.1, 0.9]`, so leaves land near the
/// drawn size and siblings stay comparable. Groups are identified by their
/// start index; leaf lookup costs `O(depth)`.
#[derive(Clone, Debug)]
pub struct SplitTree {
    key: Key,
    n: u64,
    min_leaf: u64,
    max_leaf: u64,
}

/// Maximum recursion depth; with split fractions in `[0.1, 0.9]` a leaf is
/// reached long before this for any `u64` domain.
const SPLIT_MAX_DEPTH: usize = 512;

impl SplitTree {
    /// Fragment `[0, n)` into leaves of roughly `min_leaf..=max_leaf`.
    pub fn new(n: u64, min_leaf: u64, max_leaf: u64, key: Key) -> Self {
        assert!(min_leaf >= 1 && min_leaf <= max_leaf, "invalid leaf bounds");
        Self {
            key,
            n,
            min_leaf,
            max_leaf,
        }
    }

    /// Size of the partitioned range.
    pub fn len(&self) -> u64 {
        self.n
    }

    /// True if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    #[inline]
    fn node_key(&self, lo: u64, hi: u64) -> Key {
        self.key.with2(lo, hi)
    }

    /// Split point of an internal node, or `None` if it is a leaf.
    #[inline]
    fn split(&self, lo: u64, hi: u64) -> Option<u64> {
        let size = hi - lo;
        let k = self.node_key(lo, hi);
        let leaf_size = self.min_leaf + k.with(0).below(self.max_leaf - self.min_leaf + 1);
        if size <= leaf_size {
            return None;
        }
        // Triangular on [0.1, 0.9]: mean of two uniforms, rescaled.
        let frac = 0.1 + 0.8 * 0.5 * (k.with(1).unit() + k.with(2).unit());
        let cut = lo + ((size as f64 * frac) as u64).clamp(1, size - 1);
        Some(cut)
    }

    /// The leaf containing `i`, as its index range. Panics if `i >= len`.
    pub fn leaf_of(&self, i: u64) -> Range<u64> {
        assert!(i < self.n, "SplitTree::leaf_of: {i} out of range");
        let (mut lo, mut hi) = (0u64, self.n);
        for _ in 0..SPLIT_MAX_DEPTH {
            match self.split(lo, hi) {
                None => return lo..hi,
                Some(cut) => {
                    if i < cut {
                        hi = cut;
                    } else {
                        lo = cut;
                    }
                }
            }
        }
        unreachable!("SplitTree exceeded max depth");
    }

    /// Visit every leaf in index order. Cost is linear in the number of
    /// leaves; meant for validation and small ranges.
    pub fn for_each_leaf(&self, mut f: impl FnMut(Range<u64>)) {
        let mut stack = vec![(0u64, self.n)];
        while let Some((lo, hi)) = stack.pop() {
            if lo >= hi {
                continue;
            }
            match self.split(lo, hi) {
                None => f(lo..hi),
                Some(cut) => {
                    stack.push((cut, hi));
                    stack.push((lo, cut));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perm::{Bijection, FeistelPerm};

    fn systematic(n: u64, w: &[f64], key: Key) -> Vec<u64> {
        let mut out = vec![0; w.len()];
        apportion_systematic(n, w, key, &mut out);
        out
    }

    #[test]
    fn systematic_parts_are_exact_and_within_one() {
        let w = [0.6, 0.0, 0.18, 0.12, 0.07, 1e-9, 0.03];
        let total: f64 = w.iter().sum();
        for n in [0u64, 1, 2, 3, 17, 1000, 123_457, 4_000_000_000] {
            for seed in 0..50 {
                let v = systematic(n, &w, Key::from_seed(seed));
                assert_eq!(v.iter().sum::<u64>(), n, "n {n}");
                assert_eq!(v[1], 0, "zero weight");
                for (&p, &x) in w.iter().zip(&v) {
                    assert!((x as f64 - n as f64 * p / total).abs() < 1.0 + 1e-6);
                }
            }
        }
    }

    #[test]
    fn systematic_small_splits_follow_the_weights() {
        // One member lands in class c with probability p_c: the property
        // largest remainder lacks.
        let w = [0.6, 0.25, 0.1, 0.05];
        let trials = 40_000u64;
        let mut hits = [0u64; 4];
        for seed in 0..trials {
            let v = systematic(1, &w, Key::from_seed(seed));
            hits[v.iter().position(|&x| x == 1).unwrap()] += 1;
        }
        for (h, p) in hits.iter().zip(w) {
            let f = *h as f64 / trials as f64;
            assert!((f - p).abs() < 0.01, "{f} vs {p}");
        }
        // And the mean of every part is n·p for small n.
        let mut sums = [0u64; 4];
        for seed in 0..trials {
            for (s, x) in sums.iter_mut().zip(systematic(3, &w, Key::from_seed(seed))) {
                *s += x;
            }
        }
        for (s, p) in sums.iter().zip(w) {
            let mean = *s as f64 / trials as f64;
            assert!((mean - 3.0 * p).abs() < 0.02, "{mean} vs {}", 3.0 * p);
        }
    }

    #[test]
    fn precomputed_shares_give_the_same_parts() {
        let weights: [&[f64]; 5] = [
            &[0.6, 0.0, 0.18, 0.12, 0.07, 1e-9, 0.03],
            &[0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, -2.0, f64::NAN],
            &[0.3; 41],
            &[1e-12, 5.0, 1e-12, 1e-12, 0.0, 7.0, 0.0, 1e-3],
        ];
        for w in weights {
            let shares = SystematicShares::new(w);
            assert_eq!(shares.len(), w.len());
            let mut dense = vec![0; w.len()];
            for n in (0u64..60).chain([97, 1000, 123_457, 4_000_000_000]) {
                for seed in 0..40 {
                    let key = Key::from_seed(seed ^ n << 8);
                    let want = systematic(n, w, key);
                    shares.apportion(n, key, &mut dense);
                    assert_eq!(dense, want, "dense, n {n}");
                    let mut sparse = vec![0; w.len()];
                    let mut last = None;
                    shares.for_each_part(n, key, |i, c| {
                        assert!(c > 0 && last < Some(i), "nonzero, in order");
                        last = Some(i);
                        sparse[i] = c;
                    });
                    assert_eq!(sparse, want, "sparse, n {n}");
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "no positive weight")]
    fn systematic_needs_a_positive_weight() {
        systematic(3, &[0.0, -1.0, f64::NAN], Key::from_seed(1));
    }

    #[test]
    fn systematic_golden() {
        // Pinned: the ledger's counts depend on these exact values.
        let w = [0.55, 0.2, 0.15, 0.1];
        assert_eq!(systematic(10, &w, Key::from_seed(7)), GOLDEN_SYSTEMATIC[0]);
        assert_eq!(systematic(3, &w, Key::from_seed(8)), GOLDEN_SYSTEMATIC[1]);
        assert_eq!(
            systematic(1_000_003, &w, Key::from_seed(9)),
            GOLDEN_SYSTEMATIC[2]
        );
    }

    const GOLDEN_SYSTEMATIC: [[u64; 4]; 3] = [
        [6, 2, 1, 1],
        [1, 1, 0, 1],
        [550_001, 200_001, 150_000, 100_001],
    ];
    #[test]
    fn contingency_has_exact_margins_and_the_right_means() {
        let rows = [7u64, 0, 30, 1, 12];
        let cols = [20u64, 3, 0, 27];
        let n: u64 = rows.iter().sum();
        let mut sums = vec![0u64; rows.len() * cols.len()];
        let trials = 20_000;
        for seed in 0..trials {
            let mut t = vec![0u64; rows.len() * cols.len()];
            contingency_systematic(&rows, &cols, Key::from_seed(seed), &mut t);
            for (r, &m) in rows.iter().enumerate() {
                assert_eq!(
                    t[r * cols.len()..(r + 1) * cols.len()].iter().sum::<u64>(),
                    m
                );
            }
            for (c, &k) in cols.iter().enumerate() {
                assert_eq!(
                    (0..rows.len()).map(|r| t[r * cols.len() + c]).sum::<u64>(),
                    k
                );
            }
            for (s, x) in sums.iter_mut().zip(&t) {
                *s += x;
            }
        }
        for (r, &m) in rows.iter().enumerate() {
            for (c, &k) in cols.iter().enumerate() {
                let mean = sums[r * cols.len() + c] as f64 / trials as f64;
                let want = m as f64 * k as f64 / n as f64;
                assert!(
                    (mean - want).abs() < 0.05 + 0.01 * want,
                    "{r},{c}: {mean} vs {want}"
                );
            }
        }
    }

    #[test]
    fn size_classes_tile_the_range_exactly() {
        let sc = SizeClasses::from_histogram(&[(3, 4), (1, 5), (7, 2), (3, 1)]);
        assert_eq!(sc.len(), 3 * 5 + 5 + 14);
        assert_eq!(sc.num_groups(), 5 + 5 + 2);
        assert_eq!(sc.histogram(), vec![(1, 5), (3, 5), (7, 2)]);
        let mut covered = 0u64;
        for g in 0..sc.num_groups() {
            let r = sc.range_of(g);
            assert_eq!(r.start, covered, "contiguous");
            for (slot, i) in r.clone().enumerate() {
                assert_eq!(sc.group_of(i), (g, slot as u64));
            }
            covered = r.end;
        }
        assert_eq!(covered, sc.len());
    }

    #[test]
    fn size_classes_with_permutation_are_consistent_both_ways() {
        let sc = SizeClasses::from_histogram(&[(1, 30), (2, 40), (4, 20), (9, 3)]);
        let p = FeistelPerm::new(sc.len(), Key::from_seed(3));
        for x in 0..sc.len() {
            let (g, _) = sc.group_of(p.fwd(x));
            assert!(sc.range_of(g).map(|i| p.inv(i)).any(|m| m == x));
        }
    }

    #[test]
    fn pmf_histogram_covers_exactly_n() {
        let pmf = [
            (1u64, 0.295),
            (2, 0.345),
            (3, 0.15),
            (4, 0.123),
            (5, 0.055),
            (6, 0.021),
            (7, 0.011),
        ];
        for n in [0u64, 1, 7, 100, 12_345, 10_000_000] {
            let h = histogram_from_pmf(n, &pmf);
            let covered: u64 = h.iter().map(|&(s, c)| s * c).sum();
            assert_eq!(covered, n, "n = {n}");
        }
        // Shares land close to the pmf for large n.
        let h = histogram_from_pmf(10_000_000, &pmf);
        let groups: u64 = h.iter().map(|&(_, c)| c).sum();
        let singles = h
            .iter()
            .filter(|&&(s, _)| s == 1)
            .map(|&(_, c)| c)
            .sum::<u64>();
        assert!((singles as f64 / groups as f64 - 0.295).abs() < 0.001);
    }

    #[test]
    fn split_tree_leaves_tile_and_respect_bounds() {
        let t = SplitTree::new(100_000, 20, 60, Key::from_seed(9));
        let mut next = 0u64;
        let mut sizes = Vec::new();
        t.for_each_leaf(|r| {
            assert_eq!(r.start, next);
            next = r.end;
            sizes.push(r.end - r.start);
        });
        assert_eq!(next, 100_000);
        assert!(sizes.iter().all(|&s| s <= 60));
        let mean = sizes.iter().sum::<u64>() as f64 / sizes.len() as f64;
        assert!((15.0..=60.0).contains(&mean), "mean leaf size {mean}");
        for i in (0..100_000).step_by(997) {
            let r = t.leaf_of(i);
            assert!(r.contains(&i));
        }
    }
}
