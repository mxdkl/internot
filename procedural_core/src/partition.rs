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
//! - [`round_systematic_cumulative`] and [`SparseCounts`]: keyed systematic
//!   rounding of a real-valued row (given by cumulative weights) to sparse
//!   integer counts, for wide matrices whose entries are mostly zero.
//! - [`sweep_capped`], [`apportion_largest_remainder`],
//!   [`apportion_largest_remainder_capped`] and [`trim_largest_first`]:
//!   integer splits under caps.
//! - [`pair_group`], [`even_parts`], [`segment_offset`] and
//!   [`locate_in_segments`]: small groupings and offsets over consecutive
//!   ranges.
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

/// `⌊a·b/c⌋` exactly, in 64-bit arithmetic when `a·b` fits (else 128-bit).
#[inline]
pub fn mul_div(a: u64, b: u64, c: u64) -> u64 {
    match a.checked_mul(b) {
        Some(p) => p / c,
        None => (a as u128 * b as u128 / c as u128) as u64,
    }
}

/// `⌈a·b/c⌉` exactly, in 64-bit arithmetic when `a·b` fits (else 128-bit).
#[inline]
pub fn mul_div_ceil(a: u64, b: u64, c: u64) -> u64 {
    match a.checked_mul(b) {
        Some(p) => p.div_ceil(c),
        None => (a as u128 * b as u128).div_ceil(c as u128) as u64,
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

    /// The part holding position `v` (`< n`) of the split of `n` that
    /// [`Self::apportion`] gives: the first part whose cumulative end
    /// `⌊(n·F + u)/ONE⌋` passes `v`. One binary search, no allocation.
    #[inline]
    pub fn part_of(&self, n: u64, key: Key, v: u64) -> usize {
        self.part_of_offset(n, Self::offset(key), v)
    }

    /// The systematic offset a split under `key` uses: store it where the
    /// same key repeats ([`Self::part_of_offset`], [`Self::end_of_offset`]).
    #[inline]
    pub fn offset(key: Key) -> u64 {
        key.below(SHARE_ONE as u64)
    }

    /// [`Self::part_of`] given the key's [`Self::offset`]: the same part.
    #[inline]
    pub fn part_of_offset(&self, n: u64, u: u64, v: u64) -> usize {
        debug_assert!(v < n, "position {v} of {n}");
        let u = u as u128;
        let target = (v as u128 + 1) * SHARE_ONE;
        let n128 = n as u128;
        self.cum.partition_point(|&c| n128 * c as u128 + u < target)
    }

    /// The cumulative end of parts `0..=i` of the split of `n`: positions
    /// below it lie in those parts.
    #[inline]
    pub fn end_of(&self, n: u64, key: Key, i: usize) -> u64 {
        self.end_of_offset(n, Self::offset(key), i)
    }

    /// [`Self::end_of`] given the key's [`Self::offset`]: the same end.
    #[inline]
    pub fn end_of_offset(&self, n: u64, u: u64, i: usize) -> u64 {
        if n == 0 {
            return 0;
        }
        ((n as u128 * self.cum[i] as u128 + u as u128) / SHARE_ONE) as u64
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

/// The group of position `q` when `[0, n)` is cut into pairs
/// `(2k, 2k + 1)`, with the last three positions of an odd `n` as one
/// triple, or the lone position when `n ≤ 1`: `(first position, size)`.
/// Every group has two or three members unless `n ≤ 1`.
#[inline]
pub fn pair_group(q: u64, n: u64) -> (u64, u64) {
    if n <= 1 {
        (q, 1)
    } else if n % 2 == 1 && q >= n - 3 {
        (n - 3, 3)
    } else {
        (q & !1, 2)
    }
}

/// `n` split into the fewest parts of at most `max`, as even as possible,
/// larger parts first: `k = ⌈n / max⌉` parts of `⌊n / k⌋` or one more.
/// Empty if `n` is 0 or the parts would be smaller than `min`.
pub fn even_parts(n: u64, min: u64, max: u64) -> impl Iterator<Item = u64> {
    assert!(max >= 1 && min <= max, "a valid part size range");
    let k = n.div_ceil(max);
    let (q, r) = (n.checked_div(k).unwrap_or(0), n.checked_rem(k).unwrap_or(0));
    let k = if q < min.max(1) { 0 } else { k };
    (0..k).map(move |i| q + (i < r) as u64)
}

/// The offset of segment `id` among consecutive segments `(id, size)`: the
/// sum of the sizes before it, or `None` if absent.
#[inline]
pub fn segment_offset<K: PartialEq>(
    segments: impl IntoIterator<Item = (K, u64)>,
    id: K,
) -> Option<u64> {
    let mut acc = 0;
    for (k, size) in segments {
        if k == id {
            return Some(acc);
        }
        acc += size;
    }
    None
}

/// The segment among consecutive segments `(id, size)` holding index `i`,
/// and the offset within it; `None` past the end.
#[inline]
pub fn locate_in_segments<K>(
    segments: impl IntoIterator<Item = (K, u64)>,
    mut i: u64,
) -> Option<(K, u64)> {
    for (k, size) in segments {
        if i < size {
            return Some((k, i));
        }
        i -= size;
    }
    None
}

/// Keyed unbiased rounding of one nonnegative real: `⌊x + u⌋` for one keyed
/// `u ∈ [0, 1)`, so the result is `⌊x⌋` or `⌈x⌉` and its mean over `u` is
/// exactly `x` (systematic sampling with one point). With the same `u`,
/// rounding `x − k` for an integer `k ≤ x` gives exactly `round(x) − k`, so a
/// capacity drawn down by integer takings stays one rounding. Non-positive
/// or NaN `x` gives 0.
#[inline]
pub fn round_unbiased(x: f64, u: f64) -> u64 {
    if x.is_nan() || x <= 0.0 {
        return 0;
    }
    (x + u).floor() as u64
}

/// Keyed systematic rounding of a real-valued row given by its cumulative
/// weights: the row's entries are `scale · (cum[j] − cum[j−1])`, and points
/// `u, u + 1, u + 2, …` (one keyed `u ∈ [0, 1)`) below `scale · cum[last]`
/// each land in the first entry whose scaled cumulative weight passes them.
/// `f(j, count)` receives the nonzero counts in column order.
///
/// Every entry gets the floor or the ceiling of its expectation, its mean
/// over `u` is exactly its expectation, and the row's total is within one of
/// its expectation (Madow's systematic sampling). Cost `O(points · log len)`,
/// independent of the zero entries, so a sparse row of a wide matrix is
/// cheap. `cum` must be nondecreasing; a non-positive or NaN total gives
/// nothing.
#[inline]
pub fn round_systematic_cumulative(cum: &[f64], scale: f64, u: f64, mut f: impl FnMut(usize, u64)) {
    let Some(&last) = cum.last() else {
        return;
    };
    let total = scale * last;
    if total.is_nan() || total <= 0.0 {
        return;
    }
    let len = cum.len();
    let (mut p, mut run) = (u, None::<(usize, u64)>);
    while p < total {
        let j = cum.partition_point(|&x| x * scale <= p).min(len - 1);
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

/// Integer counts as sparse rows (compressed sparse rows): row `i`'s entries
/// are `start[i]..start[i + 1]`, each a column and a count, columns
/// increasing within a row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseCounts {
    pub start: Vec<u32>,
    pub col: Vec<u32>,
    pub n: Vec<u64>,
}

impl SparseCounts {
    /// Build from rows of `(column, count)` runs in column order.
    pub fn from_rows(rows: impl IntoIterator<Item = Vec<(u32, u64)>>) -> Self {
        let rows = rows.into_iter();
        let mut x = SparseCounts {
            start: Vec::with_capacity(rows.size_hint().0 + 1),
            col: Vec::new(),
            n: Vec::new(),
        };
        for row in rows {
            x.start.push(x.col.len() as u32);
            for (j, k) in row {
                x.col.push(j);
                x.n.push(k);
            }
        }
        x.start.push(x.col.len() as u32);
        x
    }

    /// Number of rows (zero for the empty default).
    pub fn rows(&self) -> usize {
        self.start.len().saturating_sub(1)
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.col.is_empty()
    }

    /// Row `i`'s entry range.
    #[inline]
    pub fn row(&self, i: usize) -> Range<usize> {
        self.start[i] as usize..self.start[i + 1] as usize
    }

    /// The entries by column (a counting sort): column `j`'s entries are
    /// `by_col[col_start[j]..col_start[j + 1]]`, each `(row, entry index)`,
    /// rows ascending. Returns `(col_start, by_col)`.
    pub fn column_index(&self, cols: usize) -> (Vec<u32>, Vec<(u32, u32)>) {
        let mut col_start = vec![0u32; cols + 1];
        for &j in &self.col {
            col_start[j as usize + 1] += 1;
        }
        for j in 0..cols {
            col_start[j + 1] += col_start[j];
        }
        let mut by_col = vec![(0u32, 0u32); self.col.len()];
        let mut fill = col_start.clone();
        for i in 0..self.rows() {
            for e in self.row(i) {
                let j = self.col[e] as usize;
                by_col[fill[j] as usize] = (i as u32, e as u32);
                fill[j] += 1;
            }
        }
        (col_start, by_col)
    }
}

/// Keyed systematic split of `n` over `len` items by weight, with caps.
///
/// Item `k` is `item(k) = (weight, cap)`. `n` points spaced `W / n` apart
/// from offset `u · W / n` (`u ∈ [0, 1)`, keyed) fall into the items'
/// weight intervals. An item over its cap passes the excess to the next; a
/// second pass gives what remains to items with room, in order.
/// `give(k, count)` receives the counts (an item can get two calls, one per
/// pass). With no positive weight, the caps are the weights. Exact, unbiased
/// where no cap binds, linear, and allocation-light.
///
/// Panics if `n` exceeds the caps' sum.
pub fn sweep_capped(
    n: u64,
    len: usize,
    item: impl Fn(usize) -> (f64, u64),
    u: f64,
    mut give: impl FnMut(usize, u64),
) {
    let total: f64 = (0..len).map(|k| item(k).0).sum();
    let by_cap = total <= 0.0;
    let weight = |k: usize| {
        let (w, c) = item(k);
        if by_cap {
            c as f64
        } else {
            w
        }
    };
    let total = if by_cap {
        (0..len).map(|k| item(k).1 as f64).sum()
    } else {
        total
    };
    let step = total / n as f64;
    let (mut next, mut cum, mut placed, mut carry) = (u * step, 0.0, 0u64, 0u64);
    let mut given = Vec::with_capacity(len);
    let mut room_left = false;
    for k in 0..len {
        cum += weight(k);
        let mut c = carry;
        while placed < n && next < cum {
            c += 1;
            placed += 1;
            next += step;
        }
        let cap = item(k).1;
        let g = c.min(cap);
        carry = c - g;
        if g > 0 {
            give(k, g);
        }
        room_left |= g < cap;
        given.push(g);
    }
    // Points lost to rounding at the end, and excess carried past the last
    // item: to items with room, in order.
    let mut rest = carry + (n - placed);
    if rest > 0 && room_left {
        for (k, &g0) in given.iter().enumerate() {
            if rest == 0 {
                break;
            }
            let room = item(k).1 - g0;
            let g = room.min(rest);
            if g > 0 {
                give(k, g);
                rest -= g;
            }
        }
    }
    assert_eq!(rest, 0, "sweep_capped: more points than room");
}

/// Split `n` into integer parts proportional to `weights` by largest
/// remainder (Hamilton's method; ties go to the lower index). The parts sum
/// to `n` when any weight is positive; non-positive weights get nothing.
///
/// Deterministic but biased on small splits (a one-member split always goes
/// to the modal part); prefer [`apportion_systematic`] where parts can be
/// small.
pub fn apportion_largest_remainder(n: u64, weights: &[f64]) -> Vec<u64> {
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

/// [`apportion_largest_remainder`] without exceeding `caps`: parts that
/// would exceed their cap are fixed at it and the rest is re-apportioned
/// among the others; when their weights run out, by spare capacity.
///
/// Panics if `n` exceeds the caps' sum.
pub fn apportion_largest_remainder_capped(n: u64, weights: &[f64], caps: &[u64]) -> Vec<u64> {
    assert_eq!(weights.len(), caps.len(), "one cap per weight");
    assert!(
        n <= caps.iter().sum::<u64>(),
        "apportion_largest_remainder_capped: more than the caps hold"
    );
    let mut out = vec![0u64; weights.len()];
    let mut open: Vec<bool> = caps.iter().map(|&c| c > 0).collect();
    let mut left = n;
    while left > 0 {
        let idx: Vec<usize> = (0..weights.len()).filter(|&i| open[i]).collect();
        let mut w: Vec<f64> = idx.iter().map(|&i| weights[i].max(0.0)).collect();
        if w.iter().all(|&x| x <= 0.0) {
            w = idx.iter().map(|&i| (caps[i] - out[i]) as f64).collect();
        }
        let add = apportion_largest_remainder(left, &w);
        let over: Vec<usize> = idx
            .iter()
            .zip(&add)
            .filter(|&(&i, &a)| a > caps[i] - out[i])
            .map(|(&i, _)| i)
            .collect();
        if over.is_empty() {
            for (&i, &a) in idx.iter().zip(&add) {
                out[i] += a;
            }
            left = 0;
        } else {
            // Fill the overflowing parts to their caps and re-apportion the
            // rest; each round closes at least one part.
            for i in over {
                left -= caps[i] - out[i];
                out[i] = caps[i];
                open[i] = false;
            }
        }
    }
    out
}

/// Reduce `cells` until they sum to at most `cap`, taking from the largest
/// cell first (ties: lowest index). Leaves `cells` unchanged if they already
/// fit.
pub fn trim_largest_first(cells: &mut [u64], cap: u64) {
    let mut sum: u64 = cells.iter().sum();
    if sum <= cap {
        return;
    }
    // Cutting the largest cell either empties it or ends the trim, so the
    // cells are taken once each in (size descending, index) order.
    let mut order: Vec<usize> = (0..cells.len()).filter(|&i| cells[i] > 0).collect();
    order.sort_unstable_by(|&a, &b| cells[b].cmp(&cells[a]).then(a.cmp(&b)));
    for i in order {
        if sum <= cap {
            break;
        }
        let cut = (sum - cap).min(cells[i]);
        cells[i] -= cut;
        sum -= cut;
    }
}

/// Items choose owners (Lemma B, Lean `CellWorld.mother_iff`): item `j` of
/// `c` goes to owner `⌊j·m/c⌋` of `m`. Nondecreasing in `j`, and onto when
/// `c ≥ m`.
#[inline]
pub fn proportional_owner(j: u64, c: u64, m: u64) -> u64 {
    (j as u128 * m as u128 / c as u128) as u64
}

/// The items owner `i` gets under [`proportional_owner`]:
/// `[⌈i·c/m⌉, ⌈(i+1)·c/m⌉)`.
#[inline]
pub fn proportional_range(i: u64, c: u64, m: u64) -> Range<u64> {
    let (c, m) = (c as u128, m as u128);
    ((i as u128 * c).div_ceil(m) as u64)..(((i as u128 + 1) * c).div_ceil(m) as u64)
}

/// Members `j < p` of the residue class mod `k` whose smallest member is
/// `r < k`.
#[inline]
pub fn residue_count(r: u64, p: u64, k: u64) -> u64 {
    if p > r { (p - r - 1) / k + 1 } else { 0 }
}

/// The `m`-th member of the residue class mod `k` starting at `r`.
#[inline]
pub fn residue_select(r: u64, m: u64, k: u64) -> u64 {
    r + m * k
}

/// The first value at or after `a` in the cyclic range `lo..=hi` where
/// `nonempty` holds, if any.
pub fn cyclic_next(lo: i32, hi: i32, a: i32, nonempty: impl Fn(i32) -> bool) -> Option<i32> {
    let next = |x: i32| if x == hi { lo } else { x + 1 };
    let mut b = a;
    loop {
        if nonempty(b) {
            return Some(b);
        }
        b = next(b);
        if b == a {
            return None;
        }
    }
}

/// The values whose [`cyclic_next`] is `a` other than `a` itself: the run
/// just before `a` (cyclically) where `nonempty` fails, nearest first.
pub fn cyclic_run_before(lo: i32, hi: i32, a: i32, nonempty: impl Fn(i32) -> bool) -> impl Iterator<Item = i32> {
    let prev = move |x: i32| if x == lo { hi } else { x - 1 };
    let mut b = prev(a);
    std::iter::from_fn(move || {
        if b == a || nonempty(b) {
            return None;
        }
        let out = b;
        b = prev(b);
        Some(out)
    })
}

/// Of `n` members placed at the quantiles `(r + ½)/n`, those below the
/// fraction `f`: `#{r < n : r + ½ < n·f}` with `n·f` in floating point, so
/// `⌈n·f − ½⌉` clamped to `0..=n`.
#[inline]
pub fn quantile_rank_count(n: u32, f: f64) -> u32 {
    ((n as f64 * f - 0.5).ceil().max(0.0) as u32).min(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div_matches_128_bit() {
        for seed in 0..2000u64 {
            let k = Key::from_seed(seed);
            let bits = |i: u64| 1 + k.with(i).below(63);
            let (a, b) = (k.with(10).below(1 << bits(1)), k.with(11).below(1 << bits(2)));
            let c = 1 + k.with(12).below(1 << bits(3));
            let (f, cc) = (a as u128 * b as u128 / c as u128, (a as u128 * b as u128).div_ceil(c as u128));
            if f <= u64::MAX as u128 {
                assert_eq!(mul_div(a, b, c) as u128, f);
            }
            if cc <= u64::MAX as u128 {
                assert_eq!(mul_div_ceil(a, b, c) as u128, cc);
            }
        }
    }

    #[test]
    fn shares_part_of_and_end_of_match_apportion() {
        for seed in 0..60u64 {
            let k = Key::from_seed(seed);
            let len = 1 + k.with(1).below(45) as usize;
            let w: Vec<f64> = (0..len).map(|i| if k.with2(2, i as u64).below(4) == 0 { 0.0 } else { k.with2(3, i as u64).unit() }).collect();
            if w.iter().all(|&x| x <= 0.0) {
                continue;
            }
            let sh = SystematicShares::new(&w);
            for n in [1u64, 2, 7, 100, 999, 123_457] {
                let key = k.with2(4, n);
                let mut parts = vec![0u64; len];
                sh.apportion(n, key, &mut parts);
                let mut end = 0u64;
                for i in 0..len {
                    end += parts[i];
                    assert_eq!(sh.end_of(n, key, i), end, "end of part {i}");
                }
                let mut at = 0u64;
                for (i, &c) in parts.iter().enumerate() {
                    for v in [at, at + c / 2, at + c.saturating_sub(1)] {
                        if c > 0 {
                            assert_eq!(sh.part_of(n, key, v), i, "position {v} of {n}");
                        }
                    }
                    at += c;
                }
            }
        }
    }
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

    /// Dense reference for [`round_systematic_cumulative`]: count the points
    /// `u + m` below each entry's scaled upper bound.
    fn round_dense(cum: &[f64], scale: f64, u: f64) -> Vec<u64> {
        let total = scale * cum[cum.len() - 1];
        let mut out = vec![0u64; cum.len()];
        let mut p = u;
        while p < total {
            let j = (0..cum.len())
                .find(|&j| cum[j] * scale > p)
                .unwrap_or(cum.len() - 1);
            out[j] += 1;
            p += 1.0;
        }
        out
    }

    fn cumulative(w: &[f64]) -> Vec<f64> {
        let mut acc = 0.0;
        w.iter()
            .map(|&x| {
                acc += x;
                acc
            })
            .collect()
    }

    #[test]
    fn systematic_rounding_is_floor_or_ceiling_and_sparse() {
        let w = [0.0, 2.7, 0.0, 0.0, 0.35, 1.0, 0.0, 4.25, 0.05, 0.0];
        let cum = cumulative(&w);
        for scale in [0.0, 0.1, 1.0, 1.7, 13.0] {
            for seed in 0..200 {
                let u = Key::from_seed(seed).unit();
                let mut got = vec![0u64; w.len()];
                let mut last = None;
                round_systematic_cumulative(&cum, scale, u, |j, c| {
                    assert!(c > 0 && last < Some(j), "nonzero, in order");
                    last = Some(j);
                    got[j] += c;
                });
                assert_eq!(got, round_dense(&cum, scale, u));
                for (&x, &c) in w.iter().zip(&got) {
                    let e = scale * x;
                    assert!(c == e.floor() as u64 || c == e.ceil() as u64, "{c} vs {e}");
                }
                let total: u64 = got.iter().sum();
                let e = scale * cum[cum.len() - 1];
                assert!((total as f64 - e).abs() < 1.0 + 1e-9);
            }
        }
    }

    #[test]
    fn systematic_rounding_is_unbiased() {
        let w = [0.3, 0.05, 1.2, 0.45];
        let cum = cumulative(&w);
        let scale = 0.8;
        let trials = 40_000u64;
        let mut sums = [0u64; 4];
        for seed in 0..trials {
            round_systematic_cumulative(&cum, scale, Key::from_seed(seed).unit(), |j, c| {
                sums[j] += c
            });
        }
        for (s, x) in sums.iter().zip(w) {
            let mean = *s as f64 / trials as f64;
            assert!((mean - scale * x).abs() < 0.01, "{mean} vs {}", scale * x);
        }
    }

    #[test]
    fn systematic_rounding_handles_empty_and_degenerate_rows() {
        let mut calls = 0;
        round_systematic_cumulative(&[], 1.0, 0.5, |_, _| calls += 1);
        round_systematic_cumulative(&[0.0, 0.0], 5.0, 0.0, |_, _| calls += 1);
        round_systematic_cumulative(&[1.0, f64::NAN], 5.0, 0.0, |_, _| calls += 1);
        round_systematic_cumulative(&[1.0, 2.0], -1.0, 0.0, |_, _| calls += 1);
        assert_eq!(calls, 0);
    }

    #[test]
    fn unbiased_rounding_is_exact_on_average_and_consistent() {
        // The mean over a uniform grid of `u` is `x` to the grid's
        // resolution; every result is the floor or the ceiling.
        let grid = 1 << 12;
        for &x in &[0.0, 0.3, 1.0, 2.75, 7.125, 1e6 + 0.5] {
            let mut sum = 0u64;
            for k in 0..grid {
                let u = (k as f64 + 0.5) / grid as f64;
                let r = round_unbiased(x, u);
                assert!(r == x.floor() as u64 || r == x.ceil() as u64, "{x} {u}");
                sum += r;
            }
            assert!(
                (sum as f64 / grid as f64 - x).abs() <= 1.0 / grid as f64,
                "{x}"
            );
        }
        // Drawing a capacity down by integer takings keeps one rounding.
        for seed in 0..200u64 {
            let u = Key::from_seed(seed).unit();
            let x = 5.0 + seed as f64 / 37.0;
            for k in 0..5u64 {
                assert_eq!(round_unbiased(x - k as f64, u), round_unbiased(x, u) - k);
            }
        }
        assert_eq!(round_unbiased(-1.0, 0.9), 0);
        assert_eq!(round_unbiased(f64::NAN, 0.9), 0);
        assert_eq!(round_unbiased(0.0, 0.999), 0);
    }

    #[test]
    fn unbiased_rounding_golden() {
        let got: Vec<u64> = (0..6u64)
            .map(|s| round_unbiased(2.0 + s as f64 * 0.17, Key::from_seed(s).unit()))
            .collect();
        assert_eq!(got, GOLDEN_UNBIASED);
    }

    const GOLDEN_UNBIASED: [u64; 6] = [2, 2, 3, 3, 2, 3];

    #[test]
    fn systematic_rounding_golden() {
        let cum = cumulative(&[0.5, 0.0, 2.25, 1.0, 0.125, 3.0]);
        let mut got = Vec::new();
        round_systematic_cumulative(&cum, 1.5, Key::from_seed(5).unit(), |j, c| got.push((j, c)));
        assert_eq!(got, GOLDEN_ROUNDING);
    }

    const GOLDEN_ROUNDING: [(usize, u64); 3] = [(2, 4), (3, 1), (5, 5)];

    #[test]
    fn sparse_counts_index_by_column() {
        let x = SparseCounts::from_rows(vec![
            vec![(0, 2), (3, 1)],
            vec![],
            vec![(1, 5), (3, 4)],
            vec![(0, 7)],
        ]);
        assert_eq!(x.rows(), 4);
        assert_eq!(x.row(1), 2..2);
        assert_eq!(x.row(2), 2..4);
        let (start, by_col) = x.column_index(4);
        assert_eq!(start, vec![0, 2, 3, 3, 5]);
        assert_eq!(by_col, vec![(0, 0), (3, 4), (2, 2), (0, 1), (2, 3)]);
        assert!(SparseCounts::default().is_empty());
        assert_eq!(SparseCounts::default().rows(), 0);
    }

    #[test]
    fn sweep_is_exact_capped_and_proportional() {
        let weights = [0.5, 0.0, 2.0, 1.0, 0.25];
        let caps = [3u64, 4, 2, 5, 9];
        for n in 0..=caps.iter().sum::<u64>() {
            for seed in 0..40 {
                let mut got = [0u64; 5];
                sweep_capped(
                    n,
                    5,
                    |k| (weights[k], caps[k]),
                    Key::from_seed(seed).unit(),
                    |k, c| got[k] += c,
                );
                assert_eq!(got.iter().sum::<u64>(), n);
                assert!(got.iter().zip(&caps).all(|(g, c)| g <= c));
            }
        }
        // Small takings follow the weights on average.
        let mut sums = [0u64; 5];
        let trials = 20_000;
        for seed in 0..trials {
            sweep_capped(
                1,
                5,
                |k| (weights[k], caps[k]),
                Key::from_seed(seed).unit(),
                |k, c| sums[k] += c,
            );
        }
        let total: f64 = weights.iter().sum();
        for (s, w) in sums.iter().zip(weights) {
            assert!((*s as f64 / trials as f64 - w / total).abs() < 0.01);
        }
        // No positive weight: the caps are the weights.
        let mut got = [0u64; 3];
        sweep_capped(6, 3, |k| (0.0, [1, 2, 3][k]), 0.5, |k, c| got[k] += c);
        assert_eq!(got, [1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "more points than room")]
    fn sweep_needs_room() {
        sweep_capped(4, 2, |_| (1.0, 1), 0.5, |_, _| {});
    }

    #[test]
    fn sweep_golden() {
        let weights = [0.4, 1.5, 0.0, 0.75, 2.0];
        let caps = [2u64, 3, 1, 9, 4];
        let mut got = [0u64; 5];
        sweep_capped(
            9,
            5,
            |k| (weights[k], caps[k]),
            Key::from_seed(21).unit(),
            |k, c| got[k] += c,
        );
        assert_eq!(got, GOLDEN_SWEEP);
    }

    const GOLDEN_SWEEP: [u64; 5] = [1, 2, 0, 2, 4];

    #[test]
    fn largest_remainder_is_exact_and_proportional() {
        let apportion = apportion_largest_remainder;
        assert_eq!(apportion(10, &[1.0, 1.0, 1.0]), vec![4, 3, 3]);
        assert_eq!(apportion(0, &[1.0, 2.0]), vec![0, 0]);
        assert_eq!(apportion(7, &[0.0, 1.0]), vec![0, 7]);
        assert_eq!(apportion(7, &[0.0, -1.0]), vec![0, 0]);
        for n in [1u64, 13, 999, 123_457] {
            let w = [0.2, 0.5, 0.3, 0.0, 1e-9];
            let v = apportion(n, &w);
            assert_eq!(v.iter().sum::<u64>(), n);
            for (&x, &p) in v.iter().zip(&w) {
                assert!((x as f64 - n as f64 * p / 1.000000001).abs() < 1.0);
            }
        }
    }

    #[test]
    fn capped_largest_remainder_is_exact_and_capped() {
        let capped = apportion_largest_remainder_capped;
        assert_eq!(capped(10, &[1.0, 1.0], &[2, 20]), vec![2, 8]);
        assert_eq!(capped(5, &[0.0, 0.0], &[3, 3]), vec![3, 2]);
        assert_eq!(capped(0, &[1.0], &[0]), vec![0]);
        let caps = [5u64, 0, 7, 100, 1];
        for n in 0..=113 {
            let v = capped(n, &[3.0, 9.0, 0.5, 0.1, 2.0], &caps);
            assert_eq!(v.iter().sum::<u64>(), n);
            assert!(v.iter().zip(&caps).all(|(a, c)| a <= c));
        }
    }

    #[test]
    fn trim_takes_from_the_largest_first() {
        let mut v = vec![5, 9, 1, 9];
        trim_largest_first(&mut v, 10);
        assert_eq!(v.iter().sum::<u64>(), 10);
        assert_eq!(v, vec![5, 0, 1, 4]);
        let mut v = vec![1, 2];
        trim_largest_first(&mut v, 10);
        assert_eq!(v, vec![1, 2]);
        let mut v = vec![3, 3];
        trim_largest_first(&mut v, 0);
        assert_eq!(v, vec![0, 0]);
    }

    #[test]
    fn pair_groups_tile_the_range() {
        for n in 0u64..40 {
            let mut q = 0;
            while q < n {
                let (first, size) = pair_group(q, n);
                assert_eq!(first, q, "n {n}");
                assert!(n <= 1 || (2..=3).contains(&size));
                for m in first..first + size {
                    assert_eq!(pair_group(m, n), (first, size));
                }
                q += size;
            }
            assert_eq!(q, n);
        }
    }

    #[test]
    fn even_parts_are_fewest_and_even() {
        // Groups of two or three, as the roommate frames use.
        let table: [&[u64]; 13] = [
            &[],
            &[],
            &[2],
            &[3],
            &[2, 2],
            &[3, 2],
            &[3, 3],
            &[3, 2, 2],
            &[3, 3, 2],
            &[3, 3, 3],
            &[3, 3, 2, 2],
            &[3, 3, 3, 2],
            &[3, 3, 3, 3],
        ];
        for (n, want) in table.iter().enumerate() {
            assert_eq!(even_parts(n as u64, 2, 3).collect::<Vec<_>>(), *want, "{n}");
        }
        for n in 0u64..200 {
            let parts: Vec<u64> = even_parts(n, 1, 7).collect();
            assert_eq!(parts.iter().sum::<u64>(), n);
            assert_eq!(parts.len() as u64, n.div_ceil(7));
            assert!(parts.windows(2).all(|w| w[0] >= w[1] && w[0] - w[1] <= 1));
        }
    }

    #[test]
    fn segments_offset_and_locate() {
        let segs = [(7u32, 3u64), (2, 0), (9, 5), (4, 1)];
        assert_eq!(segment_offset(segs, 9), Some(3));
        assert_eq!(segment_offset(segs, 5), None);
        assert_eq!(locate_in_segments(segs, 0), Some((7, 0)));
        assert_eq!(locate_in_segments(segs, 3), Some((9, 0)));
        assert_eq!(locate_in_segments(segs, 8), Some((4, 0)));
        assert_eq!(locate_in_segments(segs, 9), None);
    }
}

#[cfg(test)]
mod cell_tests {
    use super::*;

    #[test]
    fn proportional_owners_and_ranges_are_dual() {
        for c in 1..40u64 {
            for m in 1..40u64 {
                for j in 0..c {
                    let i = proportional_owner(j, c, m);
                    assert!(i < m && proportional_range(i, c, m).contains(&j), "{c} {m} {j}");
                }
                let total: u64 = (0..m).map(|i| proportional_range(i, c, m).count() as u64).sum();
                assert_eq!(total, c);
            }
        }
    }

    #[test]
    fn residue_classes_match_brute_force() {
        for k in 1..9u64 {
            for r in 0..k {
                for p in 0..50u64 {
                    assert_eq!(residue_count(r, p, k), (0..p).filter(|j| j % k == r).count() as u64);
                    if residue_count(r, p, k) > 0 {
                        let m = residue_count(r, p, k) - 1;
                        let j = residue_select(r, m, k);
                        assert!(j < p && j % k == r && residue_count(r, j, k) == m);
                    }
                }
            }
        }
    }

    /// Lemma S: every value lands on exactly one nonempty value, and the run
    /// before a nonempty value is exactly what lands on it.
    #[test]
    fn cyclic_spill_fibers_are_runs() {
        for mask in 1u32..(1 << 9) {
            let ne = |x: i32| mask >> (x - 10) & 1 == 1;
            for a in 10..=18 {
                let t = cyclic_next(10, 18, a, ne).unwrap();
                assert!(ne(t));
                if ne(a) {
                    let run: Vec<i32> = cyclic_run_before(10, 18, a, ne).collect();
                    let fiber: Vec<i32> = (10..=18).filter(|&b| !ne(b) && cyclic_next(10, 18, b, ne) == Some(a)).collect();
                    let mut sorted = run.clone();
                    sorted.sort();
                    assert_eq!(sorted, fiber, "{mask:b} {a}");
                }
            }
        }
        assert_eq!(cyclic_next(10, 18, 12, |_| false), None);
    }

    #[test]
    fn golden() {
        assert_eq!((proportional_owner(7, 10, 3), proportional_range(1, 10, 3)), (2, 4..7));
        assert_eq!((residue_count(3, 20, 8), residue_select(3, 2, 8)), (3, 19));
        assert_eq!(cyclic_next(15, 45, 44, |x| x == 16), Some(16));
        assert_eq!(cyclic_run_before(15, 45, 16, |x| x == 16 || x == 43).collect::<Vec<_>>(), vec![15, 45, 44]);
    }
}

#[cfg(test)]
mod quantile_rank_tests {
    use super::*;

    #[test]
    fn counts_the_quantile_points_below() {
        for n in 0..40u32 {
            for i in 0..=100 {
                let f = i as f64 / 100.0;
                let brute = (0..n).filter(|&r| r as f64 + 0.5 < n as f64 * f).count() as u32;
                assert_eq!(quantile_rank_count(n, f), brute, "{n} {f}");
            }
        }
    }

    #[test]
    fn golden() {
        assert_eq!((quantile_rank_count(10, 0.26), quantile_rank_count(10, 0.25), quantile_rank_count(3, 2.0)), (3, 2, 3));
    }
}
