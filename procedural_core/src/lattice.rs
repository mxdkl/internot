//! Exact counts over lattice patterns, in O(log) time.
//!
//! - [`floor_sum`]: `Σ⌊(a·i + b)/m⌋` by a Euclid-like recursion (the
//!   AtCoder Library's `floor_sum`; Graham, Knuth & Patashnik, *Concrete
//!   Mathematics* §3). It counts lattice points under a line, which is how
//!   `perm::AffinePerm` counts how many of an interval map into an interval.
//! - [`RationalBeatty`]: an exact rational Beatty set (`t` members among
//!   `len` positions, evenly spread), with count, member and select, and the
//!   complement's select.
//! - [`ExactInterleave`]: categories of integer sizes interleaved evenly by
//!   a balanced tree of rational Beatty splits, with locate, count and
//!   select (the monotone world's husbands among each wife cohort).


/// `Σ_{i=0}^{n−1} ⌊(a·i + b)/m⌋` for `m > 0`, exactly, in O(log) steps.
///
/// Each step folds the line `y = (a·x + b)/m` onto its reflection, as
/// Euclid's algorithm does on `(a, m)`; the number of steps follows the
/// continued fraction of `a/m` until `n` runs out. Runs in 64-bit arithmetic
/// when every intermediate fits, else in 128-bit.
pub fn floor_sum(n: u64, m: u64, a: u64, b: u64) -> u128 {
    assert!(m > 0, "a positive modulus");
    if let Some(v) = floor_sum_u64(n, m, a, b) {
        return v as u128;
    }
    let (mut n, mut m, mut a, mut b) = (n as u128, m as u128, a as u128, b as u128);
    let mut ans: u128 = 0;
    loop {
        if a >= m {
            ans += n * n.saturating_sub(1) / 2 * (a / m);
            a %= m;
        }
        if b >= m {
            ans += n * (b / m);
            b %= m;
        }
        let y_max = a * n + b;
        if y_max < m {
            break;
        }
        n = y_max / m;
        b = y_max % m;
        std::mem::swap(&mut m, &mut a);
    }
    ans
}

/// [`floor_sum`] in 64 bits, or `None` if an intermediate could overflow.
#[inline]
fn floor_sum_u64(mut n: u64, mut m: u64, mut a: u64, mut b: u64) -> Option<u64> {
    // a·n + b and the partial sums stay below 2⁶³ when these hold.
    if n >= 1 << 30 || a >= 1 << 33 || b >= 1 << 33 || m >= 1 << 33 {
        return None;
    }
    let mut ans: u64 = 0;
    loop {
        if a >= m {
            ans += n * n.saturating_sub(1) / 2 * (a / m);
            a %= m;
        }
        if b >= m {
            ans += n * (b / m);
            b %= m;
        }
        let y_max = a * n + b;
        if y_max < m {
            break;
        }
        n = y_max / m;
        b = y_max % m;
        std::mem::swap(&mut m, &mut a);
    }
    Some(ans)
}

/// An exact rational Beatty set: `t ≤ len` members among `len` positions,
/// offset `tau < len`. Position `n` is a member iff `C(n + 1) > C(n)` with
/// `C(n) = ⌊(n·t + τ)/len⌋`, so exactly `t` members, at most one per
/// position, and count and select are inverse on members (Lean
/// `Transport.lean`, `Lipschitz.lean`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RationalBeatty {
    pub t: u64,
    pub len: u64,
    pub tau: u64,
}

impl RationalBeatty {
    /// Members among the first `n` positions.
    #[inline]
    pub fn count(&self, n: u64) -> u64 {
        // Below 2³² everywhere the product and offset fit in 64 bits.
        if (n | self.t | self.len) >> 32 == 0 {
            return (n * self.t + self.tau) / self.len;
        }
        ((n as u128 * self.t as u128 + self.tau as u128) / self.len as u128) as u64
    }

    #[inline]
    pub fn member(&self, n: u64) -> bool {
        self.count_member(n).1
    }

    /// `(count(n), member(n))` with one division: `n` is a member iff the
    /// remainder of `n·t + τ` by `len` is at least `len − t`.
    #[inline]
    pub fn count_member(&self, n: u64) -> (u64, bool) {
        if (n | self.t | self.len) >> 32 == 0 {
            let x = n * self.t + self.tau;
            let c = x / self.len;
            return (c, x - c * self.len + self.t >= self.len);
        }
        let x = n as u128 * self.t as u128 + self.tau as u128;
        let c = x / self.len as u128;
        (c as u64, x - c * self.len as u128 + self.t as u128 >= self.len as u128)
    }

    /// The position of member `j < t`.
    #[inline]
    pub fn select(&self, j: u64) -> u64 {
        if (j | self.t | self.len) >> 32 == 0 {
            return ((j + 1) * self.len - self.tau).div_ceil(self.t) - 1;
        }
        (((j as u128 + 1) * self.len as u128 - self.tau as u128).div_ceil(self.t as u128) - 1) as u64
    }

    /// The position of non-member `j < len − t`: `⌊(j·len + τ)/(len − t)⌋`
    /// (non-members among the first `n` are `⌈(n(len − t) − τ)/len⌉`).
    #[inline]
    pub fn select_out(&self, j: u64) -> u64 {
        if (j | self.len) >> 32 == 0 {
            return (j * self.len + self.tau) / (self.len - self.t);
        }
        ((j as u128 * self.len as u128 + self.tau as u128) / (self.len - self.t) as u128) as u64
    }
}

/// An exact interleaving of categories with integer sizes `c_0..c_{m−1}`
/// over positions `0..Σc` (the cell world's areas within a year, §17): a
/// balanced tree of exact rational Beatty splits, each node splitting its
/// positions between its two halves of categories by their totals. Every
/// category gets exactly its size, any prefix holds each category within
/// the tree depth of its share, and category, rank, count and select cost
/// O(log m) splits. Sizes come from `prefix(i) = c_0 + … + c_{i−1}`
/// (`prefix(0) = 0`); `key(node)` offsets each node.
pub struct ExactInterleave<P: Fn(usize) -> u64, K: Fn(u64) -> u64> {
    pub categories: usize,
    pub prefix: P,
    pub key: K,
}

impl<P: Fn(usize) -> u64, K: Fn(u64) -> u64> ExactInterleave<P, K> {
    /// The node splitting categories `lo..hi` (`node` numbers it): its
    /// left half's total among its positions.
    fn split(&self, node: u64, lo: usize, mid: usize, hi: usize) -> RationalBeatty {
        let (l, t) = ((self.prefix)(mid) - (self.prefix)(lo), (self.prefix)(hi) - (self.prefix)(lo));
        RationalBeatty { t: l, len: t.max(1), tau: (self.key)(node) % t.max(1) }
    }

    /// The category of position `p`, and its rank within the category.
    pub fn locate(&self, mut p: u64) -> (usize, u64) {
        let (mut lo, mut hi, mut node) = (0, self.categories, 1u64);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let b = self.split(node, lo, mid, hi);
            let c = b.count(p);
            if b.member(p) {
                (p, hi, node) = (c, mid, 2 * node);
            } else {
                (p, lo, node) = (p - c, mid, 2 * node + 1);
            }
        }
        (lo, p)
    }

    /// The position of rank `j` of category `a`.
    pub fn select(&self, a: usize, j: u64) -> u64 {
        // Descend to the leaf, remembering the path, then ascend.
        let (mut lo, mut hi, mut node) = (0, self.categories, 1u64);
        let mut path = [(0u64, 0usize, 0usize, 0usize, false); 64];
        let mut depth = 0;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let left = a < mid;
            path[depth] = (node, lo, mid, hi, left);
            depth += 1;
            if left {
                (hi, node) = (mid, 2 * node);
            } else {
                (lo, node) = (mid, 2 * node + 1);
            }
        }
        let mut p = j;
        for &(node, lo, mid, hi, left) in path[..depth].iter().rev() {
            let b = self.split(node, lo, mid, hi);
            p = if left { b.select(p) } else { b.select_out(p) };
        }
        p
    }

    /// Positions of category `a` among the first `p`.
    pub fn count(&self, a: usize, mut p: u64) -> u64 {
        let (mut lo, mut hi, mut node) = (0, self.categories, 1u64);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let b = self.split(node, lo, mid, hi);
            let c = b.count(p);
            if a < mid {
                (p, hi, node) = (c, mid, 2 * node);
            } else {
                (p, lo, node) = (p - c, mid, 2 * node + 1);
            }
        }
        p
    }
}

/// [`ExactInterleave`] with every node's split computed once: the same
/// positions, with no key hash, modulo or prefix reads per level, and one
/// count fewer per level in [`Self::locate`]. Each internal node stores its
/// left part and offset (16 bytes); its length is its parent's part (left)
/// or remainder (right), carried down the walk.
#[derive(Clone, Debug)]
pub struct InterleaveTable {
    categories: usize,
    /// Every category's total.
    total: u64,
    /// Internal node `i`'s (left part, offset), heap order from 1 (every
    /// internal node's index is below the categories' next power of two).
    nodes: Vec<(u64, u64)>,
}

impl InterleaveTable {
    /// Heap bytes.
    pub fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<(u64, u64)>()
    }

    /// The table of the interleave with these sizes (`prefix`) and node
    /// offsets (`key`), as [`ExactInterleave`] takes them.
    pub fn new(categories: usize, prefix: impl Fn(usize) -> u64, key: impl Fn(u64) -> u64) -> Self {
        let mut nodes = vec![(0u64, 0u64); categories.max(1).next_power_of_two()];
        let mut stack = vec![(1u64, 0usize, categories)];
        while let Some((node, lo, hi)) = stack.pop() {
            if hi - lo <= 1 {
                continue;
            }
            let mid = (lo + hi) / 2;
            let (l, t) = (prefix(mid) - prefix(lo), prefix(hi) - prefix(lo));
            nodes[node as usize] = (l, key(node) % t.max(1));
            stack.push((2 * node, lo, mid));
            stack.push((2 * node + 1, mid, hi));
        }
        Self { categories, total: prefix(categories) - prefix(0), nodes }
    }

    /// Node `node`'s split, given its subtree's total.
    #[inline]
    fn split(&self, node: usize, total: u64) -> RationalBeatty {
        let (t, tau) = self.nodes[node];
        RationalBeatty { t, len: total.max(1), tau }
    }

    /// The category of position `p`, and its rank within the category.
    #[inline]
    pub fn locate(&self, mut p: u64) -> (usize, u64) {
        let (mut lo, mut hi, mut node, mut total) = (0, self.categories, 1usize, self.total);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let b = self.split(node, total);
            let (c, member) = b.count_member(p);
            if member {
                (p, hi, node, total) = (c, mid, 2 * node, b.t);
            } else {
                (p, lo, node, total) = (p - c, mid, 2 * node + 1, total - b.t);
            }
        }
        (lo, p)
    }

    /// The position of rank `j` of category `a`.
    #[inline]
    pub fn select(&self, a: usize, j: u64) -> u64 {
        let (mut lo, mut hi, mut node, mut total) = (0, self.categories, 1usize, self.total);
        // The walk's splits, deepest last (left uninitialized: only the
        // first `depth` are written and read).
        let mut path = [std::mem::MaybeUninit::<(RationalBeatty, bool)>::uninit(); 64];
        let mut depth = 0;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let left = a < mid;
            let b = self.split(node, total);
            path[depth].write((b, left));
            depth += 1;
            if left {
                (hi, node, total) = (mid, 2 * node, b.t);
            } else {
                (lo, node, total) = (mid, 2 * node + 1, total - b.t);
            }
        }
        let mut p = j;
        for e in path[..depth].iter().rev() {
            // SAFETY: entries below `depth` were written above.
            let (b, left) = unsafe { e.assume_init_read() };
            p = if left { b.select(p) } else { b.select_out(p) };
        }
        p
    }

    /// Positions of category `a` among the first `p`.
    #[inline]
    pub fn count(&self, a: usize, mut p: u64) -> u64 {
        let (mut lo, mut hi, mut node, mut total) = (0, self.categories, 1usize, self.total);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let b = self.split(node, total);
            let c = b.count(p);
            if a < mid {
                (p, hi, node, total) = (c, mid, 2 * node, b.t);
            } else {
                (p, lo, node, total) = (p - c, mid, 2 * node + 1, total - b.t);
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    #[test]
    fn count_member_agrees_with_two_counts() {
        let key = Key::from_seed(9);
        for k in 0..20_000u64 {
            let big = k % 3 == 0;
            let len = 1 + key.with2(k, 0).below(if big { 1 << 40 } else { 5000 });
            let t = key.with2(k, 1).below(len + 1);
            let tau = key.with2(k, 2).below(len);
            let b = RationalBeatty { t, len, tau };
            let n = key.with2(k, 3).below(len);
            assert_eq!(b.count_member(n), (b.count(n), b.count(n + 1) > b.count(n)), "{b:?} {n}");
        }
    }

    fn brute_floor_sum(n: u64, m: u64, a: u64, b: u64) -> u128 {
        (0..n)
            .map(|i| (a as u128 * i as u128 + b as u128) / m as u128)
            .sum()
    }

    #[test]
    fn floor_sum_matches_brute_force() {
        let key = Key::from_seed(3);
        for k in 0..3000u64 {
            let n = key.with2(k, 0).below(300);
            let m = 1 + key.with2(k, 1).below(1000);
            let a = key.with2(k, 2).below(3000);
            let b = key.with2(k, 3).below(3000);
            assert_eq!(
                floor_sum(n, m, a, b),
                brute_floor_sum(n, m, a, b),
                "{n} {m} {a} {b}"
            );
        }
        // Large values: a 2³² modulus with a golden-ratio slope (the
        // 128-bit path).
        let (m, a) = (1u64 << 32, 2_654_435_769u64);
        assert_eq!(floor_sum(1000, m, a, m - 5), brute_floor_sum(1000, m, a, m - 5));
    }


    /// Every category gets exactly its size; locate, count and select agree
    /// with a brute-force walk; prefixes stay within the depth of the share.
    #[test]
    fn exact_interleave_is_exact_and_even() {
        for seed in 0..40u64 {
            let k = Key::from_seed(seed);
            let m = 1 + k.with(1).below(9) as usize;
            let sizes: Vec<u64> = (0..m).map(|i| if k.with2(2, i as u64).below(5) == 0 { 0 } else { k.with2(3, i as u64).below(80) }).collect();
            let pre: Vec<u64> = std::iter::once(0).chain(sizes.iter().scan(0, |acc, &c| { *acc += c; Some(*acc) })).collect();
            let n = pre[m];
            let e = ExactInterleave { categories: m, prefix: |i: usize| pre[i], key: |node: u64| k.with2(4, node).below(1 << 40) };
            let mut seen = vec![0u64; m];
            let depth = (m as f64).log2().ceil() as u64 + 1;
            for p in 0..n {
                let (a, r) = e.locate(p);
                assert_eq!(r, seen[a], "{seed}: rank of {p}");
                assert_eq!(e.select(a, r), p, "{seed}: select");
                seen[a] += 1;
                for b in 0..m {
                    assert_eq!(e.count(b, p + 1), seen[b], "{seed}: count {b} at {p}");
                    let share = (p + 1) as f64 * sizes[b] as f64 / n as f64;
                    assert!((seen[b] as f64 - share).abs() <= depth as f64 + 1.0, "{seed}: category {b} drifts at {p}");
                }
            }
            assert_eq!(seen, sizes, "{seed}");
        }
    }

    #[test]
    fn interleave_tables_equal_their_interleaves() {
        for seed in 0..60u64 {
            let k = Key::from_seed(seed);
            let m = 1 + k.with(1).below(60) as usize;
            let sizes: Vec<u64> = (0..m).map(|i| if k.with2(2, i as u64).below(4) == 0 { 0 } else { k.with2(3, i as u64).below(500) }).collect();
            let mut pre = vec![0u64];
            for &x in &sizes {
                pre.push(pre.last().unwrap() + x);
            }
            let kk = k.with(9);
            let e = ExactInterleave { categories: m, prefix: |i: usize| pre[i], key: |node: u64| kk.with(node).below(u64::MAX) };
            let t = InterleaveTable::new(m, |i: usize| pre[i], |node: u64| kk.with(node).below(u64::MAX));
            let total = pre[m];
            for p in 0..total {
                assert_eq!(t.locate(p), e.locate(p), "seed {seed}, position {p}");
            }
            for a in 0..m {
                for j in 0..sizes[a] {
                    assert_eq!(t.select(a, j), e.select(a, j));
                }
                for p in (0..=total).step_by(7) {
                    assert_eq!(t.count(a, p), e.count(a, p));
                }
            }
        }
    }

    #[test]
    fn rational_sets_count_and_select_inversely() {
        for len in 1..60u64 {
            for t in 1..=len {
                for tau in [0, len / 3, len - 1] {
                    let b = RationalBeatty { t, len, tau };
                    assert_eq!(b.count(len), t);
                    let members: Vec<u64> = (0..len).filter(|&n| b.member(n)).collect();
                    assert_eq!(members.len() as u64, t);
                    for (j, &n) in members.iter().enumerate() {
                        assert_eq!((b.select(j as u64), b.count(n)), (n, j as u64));
                    }
                }
            }
        }
    }

    #[test]
    fn rational_complements_select_their_non_members() {
        for len in 1..60u64 {
            for t in 0..len {
                for tau in [0, len / 3, len - 1] {
                    let b = RationalBeatty { t, len, tau };
                    let outs: Vec<u64> = (0..len).filter(|&n| !b.member(n)).collect();
                    assert_eq!(outs.len() as u64, len - t);
                    for (j, &n) in outs.iter().enumerate() {
                        assert_eq!(b.select_out(j as u64), n, "{t}/{len} τ {tau}: non-member {j}");
                        assert_eq!(n - b.count(n), j as u64);
                    }
                }
            }
        }
    }

    #[test]
    fn golden() {
        let r = RationalBeatty { t: 3, len: 10, tau: 4 };
        assert_eq!(((0..10).filter(|&n| r.member(n)).collect::<Vec<_>>(), r.select(2)), (vec![1, 5, 8], 8));
        assert_eq!((0..7).map(|j| r.select_out(j)).collect::<Vec<_>>(), vec![0, 2, 3, 4, 6, 7, 9]);
        let pre = [0u64, 2, 5, 6];
        let e = ExactInterleave { categories: 3, prefix: |i: usize| pre[i], key: |node: u64| node * 7 };
        // Sizes 2, 3, 1, interleaved: each category exactly its size.
        assert_eq!((0..6).map(|p| e.locate(p).0).collect::<Vec<_>>(), vec![1, 2, 0, 1, 1, 0]);
    }
}
