//! Reciprocal matchings built from keyed bijections.
//!
//! - [`Pairing`]: a uniform perfect matching of a prefix of `[0, n)`. It is
//!   `σ = π⁻¹ ∘ (⊕1) ∘ π`, an involution, so `σ(σ(x)) = x` holds by algebra,
//!   not by checking. Conjugation preserves cycle type, so a uniform `π` gives
//!   a uniform matching.
//! - [`Coupling`]: rank-range matching *between* blocks from an integer plan
//!   `M(a, b)`, the number of pairs spanning blocks `a` and `b`. Slot `j` of
//!   slice `(a, b)` is matched with slot `j` of slice `(b, a)`, so both sides
//!   compute the same pair and the slice counts equal `M` exactly. This is how
//!   union tables (two-sex marriage functions, age-gap kernels) become
//!   individual partners without search.
//!
//! See `docs/superpowers/research/2026-09-29-local-access-and-bijections.md` §3.

use crate::key::Key;
use crate::perm::{Bijection, FeistelPerm};

/// A uniform perfect matching on the first `paired` ranks of a keyed
/// permutation of `[0, n)`. Ranks at or above `paired` are unmatched.
#[derive(Clone, Debug)]
pub struct Pairing {
    perm: FeistelPerm,
    paired: u64,
}

impl Pairing {
    /// Match `paired` of the `n` elements (`paired` must be even and `<= n`).
    pub fn new(n: u64, paired: u64, key: Key) -> Self {
        assert!(paired <= n, "Pairing: paired {paired} > n {n}");
        assert!(paired % 2 == 0, "Pairing: paired {paired} must be even");
        Self {
            perm: FeistelPerm::new(n, key),
            paired,
        }
    }

    /// Domain size.
    pub fn len(&self) -> u64 {
        self.perm.len()
    }

    /// True if the domain is empty.
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// `x`'s partner, or `None` if `x` is unmatched.
    #[inline]
    pub fn partner(&self, x: u64) -> Option<u64> {
        let r = self.perm.fwd(x);
        (r < self.paired).then(|| self.perm.inv(r ^ 1))
    }

    /// The pair's id in `[0, paired / 2)`, shared by both partners.
    #[inline]
    pub fn pair_id(&self, x: u64) -> Option<u64> {
        let r = self.perm.fwd(x);
        (r < self.paired).then_some(r >> 1)
    }
}

/// Error building a [`Coupling`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CouplingError {
    /// The plan isn't `K × K` for `K` blocks.
    PlanShape { blocks: usize, cells: usize },
    /// `M(a, b) != M(b, a)`.
    NotSymmetric { a: usize, b: usize },
    /// Block `a` would need more members than it has.
    OverCommitted {
        block: usize,
        needed: u64,
        size: u64,
    },
}

impl std::fmt::Display for CouplingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlanShape { blocks, cells } => {
                write!(f, "plan has {cells} cells, expected {blocks}×{blocks}")
            }
            Self::NotSymmetric { a, b } => write!(f, "plan is not symmetric at ({a}, {b})"),
            Self::OverCommitted {
                block,
                needed,
                size,
            } => {
                write!(f, "block {block} needs {needed} members but has {size}")
            }
        }
    }
}

impl std::error::Error for CouplingError {}

/// Identity of a coupled pair, identical from both members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoupleId {
    /// Lower block index.
    pub a: u32,
    /// Higher block index (equal to `a` for within-block pairs).
    pub b: u32,
    /// Pair index within the slice.
    pub j: u64,
}

#[derive(Clone, Debug)]
struct CouplingBlock {
    perm: FeistelPerm,
    /// `offsets[b]` is where the slice matched with block `b` starts;
    /// `offsets[K]` is the number of used (matched) ranks.
    offsets: Vec<u64>,
}

/// Rank-range matching between blocks from a symmetric integer plan.
#[derive(Clone, Debug)]
pub struct Coupling {
    blocks: Vec<CouplingBlock>,
}

impl Coupling {
    /// Build from block sizes and a row-major `K × K` symmetric plan, where
    /// `plan[a * K + b]` is the number of pairs spanning blocks `a` and `b`
    /// (for `a == b`, pairs within the block).
    pub fn new(sizes: &[u64], plan: &[u64], key: Key) -> Result<Self, CouplingError> {
        let k = sizes.len();
        if plan.len() != k * k {
            return Err(CouplingError::PlanShape {
                blocks: k,
                cells: plan.len(),
            });
        }
        for a in 0..k {
            for b in (a + 1)..k {
                if plan[a * k + b] != plan[b * k + a] {
                    return Err(CouplingError::NotSymmetric { a, b });
                }
            }
        }
        let mut blocks = Vec::with_capacity(k);
        for (a, &size) in sizes.iter().enumerate() {
            let mut offsets = Vec::with_capacity(k + 1);
            let mut pos = 0u64;
            for b in 0..k {
                offsets.push(pos);
                let m = plan[a * k + b];
                pos += if a == b { 2 * m } else { m };
            }
            offsets.push(pos);
            if pos > size {
                return Err(CouplingError::OverCommitted {
                    block: a,
                    needed: pos,
                    size,
                });
            }
            blocks.push(CouplingBlock {
                perm: FeistelPerm::new(size, key.with(a as u64)),
                offsets,
            });
        }
        Ok(Self { blocks })
    }

    /// Number of blocks.
    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Matched members in `block`.
    pub fn used(&self, block: usize) -> u64 {
        *self.blocks[block]
            .offsets
            .last()
            .expect("offsets has K+1 entries")
    }

    /// Locate member `m` of `block`: (partner block, slot index in slice).
    #[inline]
    fn slice_of(&self, block: usize, m: u64) -> Option<(usize, u64)> {
        let blk = &self.blocks[block];
        let r = blk.perm.fwd(m);
        let used = *blk.offsets.last().unwrap();
        if r >= used {
            return None;
        }
        // Last slice whose start is <= r; empty slices share starts and are
        // skipped because the containing slice has a later index.
        let b = blk.offsets[..self.blocks.len()].partition_point(|&o| o <= r) - 1;
        Some((b, r - blk.offsets[b]))
    }

    /// `(block, member)`'s partner as `(block, member)`, or `None`.
    #[inline]
    pub fn partner(&self, block: usize, m: u64) -> Option<(usize, u64)> {
        let (b, j) = self.slice_of(block, m)?;
        if b == block {
            let blk = &self.blocks[block];
            Some((block, blk.perm.inv(blk.offsets[b] + (j ^ 1))))
        } else {
            let other = &self.blocks[b];
            Some((b, other.perm.inv(other.offsets[block] + j)))
        }
    }

    /// The pair's identity, identical from both members.
    #[inline]
    pub fn couple_id(&self, block: usize, m: u64) -> Option<CoupleId> {
        let (b, j) = self.slice_of(block, m)?;
        let (lo, hi) = if block <= b { (block, b) } else { (b, block) };
        let j = if b == block { j >> 1 } else { j };
        Some(CoupleId {
            a: lo as u32,
            b: hi as u32,
            j,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_is_an_involution_without_fixed_points() {
        for (n, paired) in [(10u64, 10u64), (11, 10), (1000, 600), (2, 2), (5, 0)] {
            let p = Pairing::new(n, paired, Key::from_seed(n + paired));
            let mut matched = 0;
            for x in 0..n {
                match p.partner(x) {
                    Some(y) => {
                        matched += 1;
                        assert_ne!(x, y);
                        assert_eq!(p.partner(y), Some(x));
                        assert_eq!(p.pair_id(x), p.pair_id(y));
                    }
                    None => assert_eq!(p.pair_id(x), None),
                }
            }
            assert_eq!(matched, paired);
        }
    }

    #[test]
    #[should_panic]
    fn pairing_rejects_odd_paired_count() {
        let _ = Pairing::new(10, 3, Key::from_seed(1));
    }

    fn check_coupling(sizes: &[u64], plan: &[u64]) {
        let k = sizes.len();
        let c = Coupling::new(sizes, plan, Key::from_seed(7)).unwrap();
        let mut span = vec![0u64; k * k];
        for (a, &size) in sizes.iter().enumerate() {
            let mut matched = 0;
            for m in 0..size {
                if let Some((b, m2)) = c.partner(a, m) {
                    matched += 1;
                    assert_eq!(c.partner(b, m2), Some((a, m)), "reciprocity");
                    assert_ne!((a, m), (b, m2), "self-pair");
                    assert_eq!(c.couple_id(a, m), c.couple_id(b, m2));
                    span[a * k + b] += 1;
                } else {
                    assert_eq!(c.couple_id(a, m), None);
                }
            }
            assert_eq!(matched, c.used(a));
        }
        for a in 0..k {
            for b in 0..k {
                let want = if a == b {
                    2 * plan[a * k + b]
                } else {
                    plan[a * k + b]
                };
                assert_eq!(span[a * k + b], want, "slice ({a},{b})");
            }
        }
    }

    #[test]
    fn coupling_realises_the_plan_exactly() {
        check_coupling(&[10, 12, 7], &[1, 3, 0, 3, 2, 4, 0, 4, 1]);
        check_coupling(&[100], &[50]);
        check_coupling(&[5, 5], &[0, 5, 5, 0]);
        check_coupling(&[3, 0, 4], &[0, 0, 2, 0, 0, 0, 2, 0, 0]);
    }

    #[test]
    fn coupling_rejects_bad_plans() {
        let k = Key::from_seed(1);
        assert!(matches!(
            Coupling::new(&[4, 4], &[0, 1, 2, 0], k),
            Err(CouplingError::NotSymmetric { .. })
        ));
        assert!(matches!(
            Coupling::new(&[4, 4], &[3, 0, 0, 0], k),
            Err(CouplingError::OverCommitted { block: 0, .. })
        ));
        assert!(matches!(
            Coupling::new(&[4, 4], &[0, 0, 0], k),
            Err(CouplingError::PlanShape { .. })
        ));
    }

    #[test]
    fn coupling_large_blocks_spot_check() {
        let sizes = [1_000_000u64, 900_000, 1_100_000];
        let plan = [
            200_000u64, 150_000, 50_000, 150_000, 100_000, 300_000, 50_000, 300_000, 250_000,
        ];
        let c = Coupling::new(&sizes, &plan, Key::from_seed(3)).unwrap();
        let key = Key::from_seed(4);
        for i in 0..20_000 {
            let a = (i % 3) as usize;
            let m = key.with(i).below(sizes[a]);
            if let Some((b, m2)) = c.partner(a, m) {
                assert_eq!(c.partner(b, m2), Some((a, m)));
            }
        }
    }
}
