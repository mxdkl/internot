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
