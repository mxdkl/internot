//! Fixed-point cumulative tables: draw an item by weight with one binary
//! search over `u32` thresholds.
//!
//! A [`CumTable`] stores each item's cumulative share in fixed point, the
//! last exactly `u32::MAX`, so a draw is integer-only and the same on every
//! machine once the table is built. Building sums the weights in `f64` in
//! item order, so the same weights always give the same thresholds.
//!
//! The 32-bit resolution loses items whose share is below about `2⁻³²`; that
//! is far below anything a census-sized list distinguishes (a name given to
//! one person in a billion still has share `10⁻⁹ ≫ 2.3·10⁻¹⁰`).
//!
//! Searches over memory-bound tables:
//! - [`Coarse`]: a sorted array with a coarse index (every [`COARSE`]th
//!   value), so a search reads the index (a line or two) and one segment
//!   instead of a binary search's scattered cache lines.
//! - [`CoarseRow`]: the same for a monotone predicate over a borrowed row
//!   whose coarse index is stored elsewhere ([`coarse_index`]).
//! - [`BucketIndex`]: which segment of a partition into consecutive ranges
//!   holds a value, by a direct-address bucket table over the value's top
//!   bits and a short search within the bucket.

use crate::key::Key;

/// Items with cumulative fixed-point weights; a draw is a binary search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CumTable<T> {
    items: Vec<T>,
    cum: Vec<u32>,
}

impl<T: Copy> CumTable<T> {
    /// A table over the items with positive weight, in order. `None` if no
    /// weight is positive (or the total is NaN).
    pub fn new(items: impl Iterator<Item = (T, f64)>) -> Option<Self> {
        let items: Vec<(T, f64)> = items.filter(|x| x.1 > 0.0).collect();
        let total: f64 = items.iter().map(|x| x.1).sum();
        if items.is_empty() || total.is_nan() || total <= 0.0 {
            return None;
        }
        let mut acc = 0.0;
        let mut cum = Vec::with_capacity(items.len());
        for &(_, w) in &items {
            acc += w;
            cum.push(((acc / total) * u32::MAX as f64).round() as u32);
        }
        *cum.last_mut().unwrap() = u32::MAX;
        Some(CumTable {
            items: items.iter().map(|x| x.0).collect(),
            cum,
        })
    }

    /// Number of items (those with positive weight).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Never true: a table has at least one item.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The items, in order.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// The item whose interval holds the fixed-point uniform `u`: the first
    /// whose cumulative threshold is at least `u`.
    #[inline]
    pub fn at(&self, u: u32) -> T {
        let i = self
            .cum
            .partition_point(|&c| c < u)
            .min(self.items.len() - 1);
        self.items[i]
    }

    /// An item drawn by weight with `key`'s top 32 bits.
    #[inline]
    pub fn draw(&self, key: Key) -> T {
        self.at((key.bits() >> 32) as u32)
    }
}

/// Spacing of the coarse indexes of [`Coarse`] and [`CoarseRow`].
pub const COARSE: usize = 16;

/// Length of the coarse index of an `n`-long row.
#[inline]
pub const fn coarse_len(n: usize) -> usize {
    n.div_ceil(COARSE)
}

/// Every [`COARSE`]th entry of a row: its coarse index.
pub fn coarse_index<T: Copy>(row: &[T]) -> impl Iterator<Item = T> + '_ {
    row.iter().step_by(COARSE).copied()
}

/// A sorted array with a coarse index (every [`COARSE`]th value).
#[derive(Clone, Debug, Default)]
pub struct Coarse<T> {
    values: Box<[T]>,
    index: Box<[T]>,
}

impl<T> Coarse<T> {
    /// Bytes held on the heap (values and index), for memory reports.
    pub fn heap_bytes(&self) -> usize {
        (self.values.len() + self.index.len()) * std::mem::size_of::<T>()
    }
}

impl<T: Copy + Ord> Coarse<T> {
    /// Index `values`, which must be sorted ascending.
    pub fn new(values: Vec<T>) -> Self {
        debug_assert!(values.windows(2).all(|w| w[0] <= w[1]), "sorted values");
        let index = coarse_index(&values).collect();
        Self {
            values: values.into_boxed_slice(),
            index,
        }
    }

    /// The values.
    #[inline]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Number of values.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Number of values at most `x`.
    #[inline]
    pub fn count_le(&self, x: T) -> usize {
        let c = self.index.partition_point(|&v| v <= x);
        if c == 0 {
            return 0;
        }
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(self.values.len());
        lo + self.values[lo..hi].partition_point(|&v| v <= x)
    }

    /// Number of values below `x`.
    #[inline]
    pub fn count_lt(&self, x: T) -> usize {
        let c = self.index.partition_point(|&v| v < x);
        if c == 0 {
            return 0;
        }
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(self.values.len());
        lo + self.values[lo..hi].partition_point(|&v| v < x)
    }

    /// Index of `x`, if present (the first, if repeated).
    #[inline]
    pub fn find(&self, x: T) -> Option<usize> {
        let i = self.count_lt(x);
        (self.values.get(i) == Some(&x)).then_some(i)
    }
}

/// A borrowed row with its coarse index (`coarse[k] = row[COARSE · k]`,
/// from [`coarse_index`]), for searches by a monotone predicate.
#[derive(Clone, Copy, Debug)]
pub struct CoarseRow<'a, T> {
    pub row: &'a [T],
    pub coarse: &'a [T],
}

impl<T: Copy> CoarseRow<'_, T> {
    /// The first index `≥ from` where `pred` fails, for `pred` true on a
    /// prefix of the row and false after: `from + row[from..].partition_point(pred)`.
    #[inline]
    pub fn first_fail(&self, from: usize, pred: impl Fn(T) -> bool) -> usize {
        let k = self.coarse.partition_point(|&c| pred(c));
        let first = if k == 0 {
            0
        } else {
            let lo = COARSE * (k - 1) + 1;
            let hi = (COARSE * k).min(self.row.len());
            lo + self.row[lo..hi].partition_point(|&x| pred(x))
        };
        first.max(from)
    }
}

impl<T: Copy + PartialOrd> CoarseRow<'_, T> {
    /// Number of entries at most `x`, for a row sorted ascending (the
    /// coarse index may be padded past the row with values above any `x`).
    #[inline]
    pub fn count_le(&self, x: T) -> usize {
        let c = self.coarse.partition_point(|&v| v <= x);
        if c == 0 {
            return 0;
        }
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(self.row.len());
        lo + self.row[lo..hi].partition_point(|&v| v <= x)
    }
}

/// Which consecutive segment holds a value: segments are
/// `starts[s]..starts[s + 1]` (ascending, the last entry the total). A table
/// maps each bucket of `2^shift` values to the first segment it overlaps,
/// with about four buckets per segment, so a lookup reads one table entry
/// and searches the few segments its bucket spans.
#[derive(Clone, Debug, Default)]
pub struct BucketIndex {
    shift: u32,
    table: Vec<u32>,
}

impl BucketIndex {
    /// Index the segments of `starts` (`segments + 1` entries, ascending,
    /// from 0).
    pub fn new(starts: &[u64]) -> Self {
        let segments = starts.len() - 1;
        let total = starts[segments].max(1);
        let target = (4 * segments as u64).next_power_of_two().max(1);
        let mut shift = 0;
        while (total - 1) >> shift >= target {
            shift += 1;
        }
        let buckets = (((total - 1) >> shift) + 1) as usize;
        let segment_of =
            |x: u64| (starts.partition_point(|&s| s <= x) - 1).min(segments - 1) as u32;
        let mut table: Vec<u32> = (0..buckets)
            .map(|i| segment_of((i as u64) << shift))
            .collect();
        table.push(segment_of(total - 1));
        Self { shift, table }
    }

    /// The segment of `starts` (the same as indexed) holding `x < total`.
    #[inline]
    pub fn segment(&self, starts: &[u64], x: u64) -> usize {
        let i = (x >> self.shift) as usize;
        let (lo, hi) = (self.table[i] as usize, self.table[i + 1] as usize);
        if lo == hi {
            lo
        } else {
            lo + starts[lo + 1..=hi].partition_point(|&s| s <= x)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_follow_the_weights() {
        let t = CumTable::new([(10u32, 1.0), (11, 0.0), (12, 3.0), (13, 6.0)].into_iter()).unwrap();
        assert_eq!(t.items(), &[10, 12, 13]);
        let trials = 50_000u64;
        let mut hits = [0u64; 3];
        for seed in 0..trials {
            let x = t.draw(Key::from_seed(seed));
            hits[t.items().iter().position(|&i| i == x).unwrap()] += 1;
        }
        for (h, p) in hits.iter().zip([0.1, 0.3, 0.6]) {
            assert!((*h as f64 / trials as f64 - p).abs() < 0.01);
        }
    }

    #[test]
    fn thresholds_cover_the_whole_range() {
        let t = CumTable::new((0..100u32).map(|i| (i, 1.0 + i as f64))).unwrap();
        assert_eq!(t.at(0), 0);
        assert_eq!(t.at(u32::MAX), 99);
        let mut prev = 0;
        for u in (0..=u32::MAX).step_by(1 << 20) {
            let x = t.at(u);
            assert!(x >= prev, "monotone in u");
            prev = x;
        }
    }

    #[test]
    fn empty_and_invalid_weights_give_no_table() {
        assert!(CumTable::<u8>::new(std::iter::empty()).is_none());
        assert!(CumTable::new([(1u8, 0.0), (2, -1.0)].into_iter()).is_none());
        assert!(CumTable::new([(1u8, f64::NAN)].into_iter()).is_none());
        assert!(CumTable::new([(1u8, 1.0), (2, f64::INFINITY)].into_iter()).is_some());
    }

    #[test]
    fn golden() {
        let t =
            CumTable::new([(7u32, 0.25), (8, 0.5), (9, 0.125), (10, 0.125)].into_iter()).unwrap();
        let draws: Vec<u32> = (0..8).map(|s| t.draw(Key::from_seed(s))).collect();
        assert_eq!(draws, GOLDEN_DRAWS);
    }

    const GOLDEN_DRAWS: [u32; 8] = [8, 7, 10, 10, 7, 9, 10, 9];

    #[test]
    fn coarse_search_matches_a_plain_search() {
        for n in [0usize, 1, 15, 16, 17, 100, 1000] {
            let values: Vec<i32> = (0..n as i32).map(|i| i * 3 - (i % 5)).collect();
            let mut sorted = values.clone();
            sorted.sort_unstable();
            let c = Coarse::new(sorted.clone());
            assert_eq!(c.len(), n);
            for x in -5..(3 * n as i32 + 5) {
                assert_eq!(c.count_le(x), sorted.partition_point(|&v| v <= x));
                assert_eq!(c.count_lt(x), sorted.partition_point(|&v| v < x));
                assert_eq!(
                    c.find(x),
                    sorted
                        .binary_search(&x)
                        .ok()
                        .map(|_| sorted.partition_point(|&v| v < x))
                );
            }
        }
    }

    #[test]
    fn coarse_row_matches_partition_point() {
        let row: Vec<f64> = (0..112).map(|i| 1.0 - i as f64 / 111.0).collect();
        let coarse: Vec<f64> = coarse_index(&row).collect();
        assert_eq!(coarse.len(), coarse_len(row.len()));
        let r = CoarseRow {
            row: &row,
            coarse: &coarse,
        };
        for from in 1..row.len() {
            for t in [0.0, 0.01, 0.33, 0.5, 0.99, 1.0] {
                let pred = |s: f64| s >= t;
                if !pred(row[from - 1]) {
                    continue;
                }
                assert_eq!(
                    r.first_fail(from, pred),
                    from + row[from..].partition_point(|&x| pred(x))
                );
            }
        }
        // Sorted rows: count_le, with a padded index.
        let sorted: Vec<u32> = (0..50).map(|i| i * 7 + i % 3).collect();
        let mut coarse: Vec<u32> = coarse_index(&sorted).collect();
        coarse.resize(coarse_len(sorted.len()) + 2, u32::MAX);
        let r = CoarseRow {
            row: &sorted,
            coarse: &coarse,
        };
        for x in 0..400 {
            assert_eq!(r.count_le(x), sorted.partition_point(|&v| v <= x), "{x}");
        }
    }

    #[test]
    fn buckets_find_every_segment() {
        let sizes = [5u64, 0, 1, 300, 2, 2, 0, 77, 1, 1000, 3];
        let mut starts = vec![0u64];
        for s in sizes {
            starts.push(starts.last().unwrap() + s);
        }
        let b = BucketIndex::new(&starts);
        for x in 0..*starts.last().unwrap() {
            let want = starts.partition_point(|&s| s <= x) - 1;
            assert_eq!(b.segment(&starts, x), want, "{x}");
        }
    }
}
