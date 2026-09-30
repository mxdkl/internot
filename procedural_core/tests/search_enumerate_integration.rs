//! End-to-end test: range_to_prefixes ∘ enumerate reconstructs the range.

use procedural_core::search::{range_to_prefixes, BitPattern};

fn reconstruct_range(lo: u64, hi: u64, bits: u8) -> Vec<u64> {
    let patterns = range_to_prefixes(lo, hi, bits);
    let mut all: Vec<u64> = patterns.iter().flat_map(|p| p.enumerate(bits)).collect();
    all.sort_unstable();
    all
}

#[test]
fn enumerate_reconstructs_small_range() {
    let reconstructed = reconstruct_range(3, 12, 4);
    let expected: Vec<u64> = (3..=12).collect();
    assert_eq!(reconstructed, expected);
}

#[test]
fn enumerate_reconstructs_worst_case_range() {
    // 8-bit worst case: [1, 254]
    let reconstructed = reconstruct_range(1, 254, 8);
    let expected: Vec<u64> = (1..=254).collect();
    assert_eq!(reconstructed, expected);
}

#[test]
fn enumerate_reconstructs_singleton() {
    let reconstructed = reconstruct_range(42, 42, 8);
    assert_eq!(reconstructed, vec![42]);
}

#[test]
fn enumerate_reconstructs_full_domain() {
    let reconstructed = reconstruct_range(0, 15, 4);
    let expected: Vec<u64> = (0..=15).collect();
    assert_eq!(reconstructed, expected);
}

#[test]
fn enumerate_does_not_duplicate_across_patterns() {
    // range_to_prefixes emits disjoint patterns; no ID should appear twice.
    let patterns = range_to_prefixes(1, 6, 3);
    let mut all: Vec<u64> = patterns.iter().flat_map(|p| p.enumerate(3)).collect();
    all.sort_unstable();
    let unique_count = {
        let mut uniq = all.clone();
        uniq.dedup();
        uniq.len()
    };
    assert_eq!(
        all.len(),
        unique_count,
        "duplicate IDs across patterns: {:?}",
        all
    );
}

#[test]
fn exact_pattern_enumerate_cardinality_is_one() {
    let p = BitPattern::exact(100, 8);
    assert_eq!(p.enumerate(8).cardinality(), 1);
}

#[test]
fn any_pattern_enumerate_cardinality_is_full_domain() {
    let p = BitPattern::any(6);
    assert_eq!(p.enumerate(6).cardinality(), 64);
}
