//! Canonical pair-keying for symmetric pair-hashes.
//!
//! Two-id hashing must be symmetric in `(a, b)` so reciprocity is
//! automatic: `hash(A, B) == hash(B, A)`. Pack the smaller id into
//! the high 32 bits, larger into the low — so both `(7, 42)` and
//! `(42, 7)` map to the same u64.

use crate::hash::hash_float;

/// Pack two `u32` ids into a canonical `u64`, order-independent.
pub fn canonical_pair(a: u32, b: u32) -> u64 {
    let (min_id, max_id) = if a <= b { (a, b) } else { (b, a) };
    ((min_id as u64) << 32) | (max_id as u64)
}

/// Pair-keyed `hash_float` — symmetric in `(a, b)`.
pub fn pair_hash_float(a: u32, b: u32, key: &str) -> f64 {
    hash_float(canonical_pair(a, b), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_pair_is_symmetric() {
        assert_eq!(canonical_pair(7, 42), canonical_pair(42, 7));
        assert_eq!(canonical_pair(0, u32::MAX), canonical_pair(u32::MAX, 0));
    }

    #[test]
    fn canonical_pair_distinguishes_distinct_pairs() {
        let p1 = canonical_pair(1, 2);
        let p2 = canonical_pair(2, 3);
        let p3 = canonical_pair(1, 3);
        assert_ne!(p1, p2);
        assert_ne!(p1, p3);
        assert_ne!(p2, p3);
    }

    #[test]
    fn pair_hash_float_is_symmetric() {
        let h1 = pair_hash_float(7, 42, "iet");
        let h2 = pair_hash_float(42, 7, "iet");
        assert_eq!(h1, h2);
    }

    #[test]
    fn pair_hash_float_changes_with_key() {
        let h1 = pair_hash_float(7, 42, "iet");
        let h2 = pair_hash_float(7, 42, "topic");
        assert_ne!(h1, h2);
    }
}
