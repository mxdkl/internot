//! Counted spaces: exact integer structure over a rank line.
//!
//! - [`CountTree`]: a random count spread over `2^levels` leaves by binomial
//!   splits down a dyadic tree (Goldreich–Goldwasser–Nussboim interval sums).
//!   Children always sum to their parent, so consistency is structural: it
//!   holds even if the sampler were imperfect. `count_before`, `leaf_count`
//!   and `select` each cost `O(levels)` binomial draws.
//! - [`QuotaTree`]: nested integer cells over a rank line (sex → union cell →
//!   plan cell → death cell, in the kinship design). Every margin is the sum
//!   of its children by construction, which is what lets two sides of a
//!   relation agree on counts exactly.
//!
//! See `docs/superpowers/research/2026-09-29-local-access-and-bijections.md`
//! §4.3 and `2026-09-29-kinship-and-households.md` §9.3.

use std::ops::Range;

use crate::key::Key;
use crate::sample::binomial;

/// A count spread over `2^levels` leaves by keyed binomial splits.
///
/// `weight(lo, hi)` gives the relative mass of leaf range `[lo, hi)`; a node's
/// items go left with probability `weight(left) / weight(node)`. With
/// `weight = hi - lo` the leaves are exchangeable.
pub struct CountTree<W> {
    key: Key,
    levels: u8,
    total: u64,
    weight: W,
}

impl<W: Fn(u64, u64) -> f64> CountTree<W> {
    /// A tree of `2^levels` leaves holding `total` items.
    pub fn new(key: Key, levels: u8, total: u64, weight: W) -> Self {
        assert!(levels <= 62, "CountTree supports at most 62 levels");
        Self {
            key,
            levels,
            total,
            weight,
        }
    }

    /// Number of leaves.
    pub fn leaves(&self) -> u64 {
        1u64 << self.levels
    }

    /// Total items.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Items that go to the left child of node `(depth, index)` holding `n`.
    #[inline]
    fn left_count(&self, depth: u8, index: u64, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let span = 1u64 << (self.levels - depth);
        let lo = index * span;
        let mid = lo + span / 2;
        let hi = lo + span;
        let w_left = (self.weight)(lo, mid);
        let w_all = w_left + (self.weight)(mid, hi);
        if w_all <= 0.0 {
            return 0;
        }
        let p = (w_left / w_all).clamp(0.0, 1.0);
        binomial(self.key.with2(depth as u64, index), n, p)
    }

    /// Items in leaves strictly before `leaf`, and the count in `leaf`.
    #[inline]
    pub fn prefix_and_leaf(&self, leaf: u64) -> (u64, u64) {
        assert!(leaf < self.leaves(), "CountTree: leaf {leaf} out of range");
        let (mut before, mut n, mut index) = (0u64, self.total, 0u64);
        for depth in 0..self.levels {
            let left = self.left_count(depth, index, n);
            let go_right = (leaf >> (self.levels - depth - 1)) & 1 == 1;
            if go_right {
                before += left;
                n -= left;
                index = index * 2 + 1;
            } else {
                n = left;
                index *= 2;
            }
        }
        (before, n)
    }

    /// Items in leaves strictly before `leaf`.
    pub fn count_before(&self, leaf: u64) -> u64 {
        self.prefix_and_leaf(leaf).0
    }

    /// Items in `leaf`.
    pub fn leaf_count(&self, leaf: u64) -> u64 {
        self.prefix_and_leaf(leaf).1
    }

    /// The leaf holding item `k` (0-based, in leaf order) and its position
    /// within that leaf. Panics if `k >= total`.
    pub fn select(&self, k: u64) -> (u64, u64) {
        assert!(
            k < self.total,
            "CountTree::select: {k} >= total {}",
            self.total
        );
        let (mut rest, mut n, mut index) = (k, self.total, 0u64);
        for depth in 0..self.levels {
            let left = self.left_count(depth, index, n);
            if rest < left {
                n = left;
                index *= 2;
            } else {
                rest -= left;
                n -= left;
                index = index * 2 + 1;
            }
        }
        (index, rest)
    }

    /// Visit `(leaf, count)` for every non-empty leaf in `[lo, hi)`, in order.
    /// Cost is `O(levels · non-empty leaves visited)`.
    pub fn for_each_nonempty(&self, lo: u64, hi: u64, mut f: impl FnMut(u64, u64)) {
        let hi = hi.min(self.leaves());
        if lo >= hi {
            return;
        }
        self.visit(0, 0, self.total, lo, hi, &mut f);
    }

    fn visit(&self, depth: u8, index: u64, n: u64, lo: u64, hi: u64, f: &mut impl FnMut(u64, u64)) {
        if n == 0 {
            return;
        }
        let span = 1u64 << (self.levels - depth);
        let (start, end) = (index * span, index * span + span);
        if end <= lo || start >= hi {
            return;
        }
        if depth == self.levels {
            f(index, n);
            return;
        }
        let left = self.left_count(depth, index, n);
        self.visit(depth + 1, index * 2, left, lo, hi, f);
        self.visit(depth + 1, index * 2 + 1, n - left, lo, hi, f);
    }
}

/// Maximum nesting depth of a [`QuotaTree`].
pub const QUOTA_MAX_DEPTH: usize = 8;

/// Specification of one quota cell: a label and either a count (leaf) or
/// children whose counts sum to this cell's count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotaSpec {
    /// Caller-defined label (e.g. an encoded partner cohort).
    pub label: u32,
    /// Members in this cell.
    pub count: u64,
    /// Sub-cells, in rank order. Empty for a leaf.
    pub children: Vec<QuotaSpec>,
}

impl QuotaSpec {
    /// A leaf cell.
    pub fn leaf(label: u32, count: u64) -> Self {
        Self {
            label,
            count,
            children: Vec::new(),
        }
    }

    /// An internal cell whose count is the sum of its children.
    pub fn node(label: u32, children: Vec<QuotaSpec>) -> Self {
        let count = children.iter().map(|c| c.count).sum();
        Self {
            label,
            count,
            children,
        }
    }
}

/// Error building a [`QuotaTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaError {
    /// A cell's count differs from the sum of its children.
    MarginMismatch {
        path: Vec<u32>,
        count: u64,
        children_sum: u64,
    },
    /// Nesting deeper than [`QUOTA_MAX_DEPTH`].
    TooDeep,
}

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarginMismatch {
                path,
                count,
                children_sum,
            } => write!(
                f,
                "quota cell {path:?} has count {count} but children sum to {children_sum}"
            ),
            Self::TooDeep => write!(f, "quota tree deeper than {QUOTA_MAX_DEPTH}"),
        }
    }
}

impl std::error::Error for QuotaError {}

#[derive(Clone, Copy, Debug)]
struct QNode {
    label: u32,
    start: u64,
    len: u64,
    first_child: u32,
    num_children: u32,
}

/// Where a rank falls in a [`QuotaTree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotaLocation {
    labels: [u32; QUOTA_MAX_DEPTH],
    depth: u8,
    /// Node id of the leaf cell.
    pub leaf: u32,
    /// Offset of the rank within the leaf cell.
    pub offset: u64,
}

impl QuotaLocation {
    /// Labels from the top-level cell down to the leaf.
    pub fn labels(&self) -> &[u32] {
        &self.labels[..self.depth as usize]
    }
}

/// Nested integer cells over the rank line `[0, total)`.
#[derive(Clone, Debug)]
pub struct QuotaTree {
    nodes: Vec<QNode>,
    roots: u32,
    total: u64,
}

impl QuotaTree {
    /// Build from top-level cells. Validates every margin.
    pub fn new(cells: &[QuotaSpec]) -> Result<Self, QuotaError> {
        let mut nodes = Vec::new();
        let mut path = Vec::new();
        // Breadth-first layout keeps each node's children contiguous.
        fn check(spec: &QuotaSpec, path: &mut Vec<u32>, depth: usize) -> Result<(), QuotaError> {
            if depth > QUOTA_MAX_DEPTH {
                return Err(QuotaError::TooDeep);
            }
            path.push(spec.label);
            if !spec.children.is_empty() {
                let sum: u64 = spec.children.iter().map(|c| c.count).sum();
                if sum != spec.count {
                    return Err(QuotaError::MarginMismatch {
                        path: path.clone(),
                        count: spec.count,
                        children_sum: sum,
                    });
                }
                for c in &spec.children {
                    check(c, path, depth + 1)?;
                }
            }
            path.pop();
            Ok(())
        }
        for c in cells {
            check(c, &mut path, 1)?;
        }
        // Lay out: roots first, then each node's children contiguously.
        let mut queue: std::collections::VecDeque<(&QuotaSpec, u64)> =
            std::collections::VecDeque::new();
        let mut start = 0u64;
        for c in cells {
            queue.push_back((c, start));
            start += c.count;
        }
        let total = start;
        let roots = cells.len() as u32;
        // Assign ids in BFS order; children ids are allocated when a node is
        // popped, so they are contiguous.
        let mut next_id = roots;
        while let Some((spec, st)) = queue.pop_front() {
            let first_child = next_id;
            let mut cs = st;
            for ch in &spec.children {
                queue.push_back((ch, cs));
                cs += ch.count;
            }
            next_id += spec.children.len() as u32;
            nodes.push(QNode {
                label: spec.label,
                start: st,
                len: spec.count,
                first_child,
                num_children: spec.children.len() as u32,
            });
        }
        Ok(Self {
            nodes,
            roots,
            total,
        })
    }

    /// Total ranks covered.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Locate rank `r`: the leaf cell, its label path and the offset within
    /// it. Zero-count cells are never returned. Panics if `r >= total`.
    #[inline]
    pub fn locate(&self, r: u64) -> QuotaLocation {
        assert!(
            r < self.total,
            "QuotaTree::locate: {r} >= total {}",
            self.total
        );
        let mut loc = QuotaLocation {
            labels: [0; QUOTA_MAX_DEPTH],
            depth: 0,
            leaf: 0,
            offset: 0,
        };
        let (mut first, mut count) = (0u32, self.roots);
        loop {
            let span = &self.nodes[first as usize..(first + count) as usize];
            // Last sibling whose start <= r and that is non-empty containing r.
            let idx = span.partition_point(|n| n.start + n.len <= r);
            let node = span[idx];
            let id = first + idx as u32;
            loc.labels[loc.depth as usize] = node.label;
            loc.depth += 1;
            if node.num_children == 0 {
                loc.leaf = id;
                loc.offset = r - node.start;
                return loc;
            }
            first = node.first_child;
            count = node.num_children;
        }
    }

    /// Rank range of the cell reached by following `labels` from the top.
    /// Returns `None` if the path doesn't exist. Labels are matched in order
    /// among siblings (first match).
    pub fn range(&self, labels: &[u32]) -> Option<Range<u64>> {
        let (mut first, mut count) = (0u32, self.roots);
        let mut found: Option<QNode> = None;
        for &label in labels {
            let span = &self.nodes[first as usize..(first + count) as usize];
            let node = *span.iter().find(|n| n.label == label)?;
            found = Some(node);
            first = node.first_child;
            count = node.num_children;
        }
        found.map(|n| n.start..n.start + n.len)
    }

    /// Rank range of a node id (as returned in [`QuotaLocation::leaf`]).
    pub fn node_range(&self, id: u32) -> Range<u64> {
        let n = self.nodes[id as usize];
        n.start..n.start + n.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_tree_prefix_sums_are_consistent() {
        let t = CountTree::new(Key::from_seed(1), 10, 5_000, |lo, hi| (hi - lo) as f64);
        let mut running = 0u64;
        for leaf in 0..t.leaves() {
            let (before, here) = t.prefix_and_leaf(leaf);
            assert_eq!(before, running, "leaf {leaf}");
            running += here;
        }
        assert_eq!(running, 5_000);
    }

    #[test]
    fn count_tree_select_inverts_prefix() {
        let t = CountTree::new(Key::from_seed(2), 8, 777, |lo, hi| (hi - lo) as f64);
        for k in 0..777 {
            let (leaf, pos) = t.select(k);
            let (before, here) = t.prefix_and_leaf(leaf);
            assert!(pos < here);
            assert_eq!(before + pos, k);
        }
    }

    #[test]
    fn count_tree_nonempty_visit_matches_leaf_counts() {
        let t = CountTree::new(Key::from_seed(3), 12, 300, |lo, hi| (hi - lo) as f64);
        let mut seen = 0u64;
        t.for_each_nonempty(100, 3000, |leaf, n| {
            assert!((100..3000).contains(&leaf));
            assert_eq!(t.leaf_count(leaf), n);
            seen += n;
        });
        assert_eq!(seen, t.count_before(3000) - t.count_before(100));
    }

    #[test]
    fn count_tree_respects_weights() {
        // All mass in the first half: nothing may land in the second.
        let t = CountTree::new(Key::from_seed(4), 6, 1_000, |lo, hi| {
            let hi = hi.min(32);
            hi.saturating_sub(lo) as f64
        });
        assert_eq!(t.count_before(32), 1_000);
    }

    fn sample_tree() -> Vec<QuotaSpec> {
        vec![
            QuotaSpec::node(
                0, // female
                vec![
                    QuotaSpec::node(
                        10,
                        vec![
                            QuotaSpec::leaf(100, 30),
                            QuotaSpec::leaf(101, 0),
                            QuotaSpec::leaf(102, 20),
                        ],
                    ),
                    QuotaSpec::leaf(11, 15),
                ],
            ),
            QuotaSpec::node(1, vec![QuotaSpec::leaf(20, 40), QuotaSpec::leaf(21, 5)]),
        ]
    }

    #[test]
    fn quota_locate_covers_every_rank_once() {
        let q = QuotaTree::new(&sample_tree()).unwrap();
        assert_eq!(q.total(), 110);
        let mut per_leaf = std::collections::HashMap::new();
        for r in 0..q.total() {
            let loc = q.locate(r);
            let range = q.node_range(loc.leaf);
            assert!(range.contains(&r));
            assert_eq!(r - range.start, loc.offset);
            *per_leaf.entry(loc.labels().to_vec()).or_insert(0u64) += 1;
        }
        assert_eq!(per_leaf[&vec![0, 10, 100]], 30);
        assert_eq!(
            per_leaf.get(&vec![0, 10, 101]),
            None,
            "empty cell never located"
        );
        assert_eq!(per_leaf[&vec![0, 10, 102]], 20);
        assert_eq!(per_leaf[&vec![0, 11]], 15);
        assert_eq!(per_leaf[&vec![1, 20]], 40);
        assert_eq!(per_leaf[&vec![1, 21]], 5);
    }

    #[test]
    fn quota_range_by_labels() {
        let q = QuotaTree::new(&sample_tree()).unwrap();
        assert_eq!(q.range(&[0]), Some(0..65));
        assert_eq!(q.range(&[0, 10, 102]), Some(30..50));
        assert_eq!(q.range(&[1, 21]), Some(105..110));
        assert_eq!(q.range(&[2]), None);
    }

    #[test]
    fn quota_rejects_bad_margins() {
        let bad = vec![QuotaSpec {
            label: 0,
            count: 10,
            children: vec![QuotaSpec::leaf(1, 4)],
        }];
        assert!(matches!(
            QuotaTree::new(&bad),
            Err(QuotaError::MarginMismatch { .. })
        ));
    }
}
