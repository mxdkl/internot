//! `Cohort<W>` — a bitmask-described set of ids returned from
//! `find().into_cohort()`. CIDR-style: O(1) to describe, O(1) to
//! contains-test, O(N) to sample N diverse members. Unlike the
//! find()-iterator, a Cohort doesn't enumerate at construction.
//!
//! Operations:
//! - `contains(id)` — bit-test against the underlying patterns
//! - `sample(n)` — yield N members via per-field hash-into-free-bits
//!   sampling (each field samples independently)
//!
//! v1 limitations:
//! - Filters (filter_static / filter_temporal) are not carried into
//!   Cohort. Add them via the Cohort.with_filter() builder if needed.
//! - Multi-pattern fields (from `where_in`) are not yet supported.
//!   Use `where_eq` or `where_range` constraints for now.

use std::marker::PhantomData;

use crate::search::pattern::BitPattern;
use crate::word::BitWord;

/// A bitmask-described cohort. Cheap to construct, cheap to test
/// membership, sample-able for diverse member draws.
pub struct Cohort<W: BitWord> {
    /// (offset, width, pattern) per constrained field. Fields not in
    /// this list are unconstrained — they pass any value.
    pub(crate) patterns_per_field: Vec<(u32, u32, BitPattern)>,
    pub(crate) _word: PhantomData<W>,
}

impl<W: BitWord> Cohort<W> {
    /// Test whether the given id is a member of this cohort.
    /// Cost: O(num_constrained_fields).
    pub fn contains(&self, id: W) -> bool {
        for (offset, width, pat) in &self.patterns_per_field {
            let extracted = id.extract_bits(*offset, *width);
            if !pat.matches(extracted) {
                return false;
            }
        }
        true
    }

    /// Sample N diverse members. Each sample seeds a per-field
    /// `BitPattern::sample` and packs the results into a full `W`.
    /// Deterministic: `sample(n)` always yields the same members.
    pub fn sample(&self, n: usize) -> Vec<W> {
        let mut out = Vec::with_capacity(n);
        for seed in 0..n as u64 {
            out.push(self.sample_one(seed));
        }
        out
    }

    /// Generate one member with the given seed. Pattern bits are
    /// pinned per `BitPattern::sample`; unconstrained fields stay
    /// zero (they're outside the registered patterns).
    fn sample_one(&self, seed: u64) -> W {
        let mut id = W::zero();
        for (offset, width, pat) in &self.patterns_per_field {
            let bits = pat.sample(seed, *width as u8);
            id = id.insert_bits(*offset, *width, bits);
        }
        id
    }
}
