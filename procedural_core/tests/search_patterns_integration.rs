//! End-to-end test: realistic field queries combining range and set cover.

use procedural_core::search::{minimize_patterns, range_to_prefixes, BitPattern};

/// Verify that the union of patterns from `range_to_prefixes` covers exactly
/// `[lo, hi]` — no missing, no extras.
fn assert_exact_range_cover(lo: u64, hi: u64, bits: u8) {
    let patterns = range_to_prefixes(lo, hi, bits);
    let domain = 1u64 << bits;
    for v in 0..domain {
        let c = patterns.iter().filter(|p| p.matches(v)).count();
        let expected = if (lo..=hi).contains(&v) { 1 } else { 0 };
        assert_eq!(
            c, expected,
            "v={v} lo={lo} hi={hi} bits={bits}: matched {c}, expected {expected}"
        );
    }
}

/// Verify that the union of patterns from `minimize_patterns` covers exactly
/// the input value set — no missing, no extras.
fn assert_exact_set_cover(values: &[u64], width: u8) {
    let patterns = minimize_patterns(values, width);
    let domain = 1u64 << width;
    let value_set: std::collections::HashSet<u64> = values.iter().copied().collect();
    for v in 0..domain {
        let matches = patterns.iter().any(|p| p.matches(v));
        let expected = value_set.contains(&v);
        assert_eq!(
            matches, expected,
            "v={v} width={width}: matched={matches}, expected={expected}"
        );
    }
}

#[test]
fn age_range_query_16_to_25_on_7_bit_field() {
    // Realistic: "people aged 16–25" with a 7-bit age field.
    assert_exact_range_cover(16, 25, 7);
}

#[test]
fn birth_year_offset_range_covers_exactly() {
    // 1950 + offset; querying "born 1970–1989" means offset range [20, 39] on 7 bits.
    assert_exact_range_cover(20, 39, 7);
}

#[test]
fn locale_set_membership_covers_exactly() {
    // 8-bit locale field; pick a handful of locale codes.
    let locales: Vec<u64> = vec![0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x20];
    assert_exact_set_cover(&locales, 8);
}

#[test]
fn occupation_dense_set_minimizes() {
    // 4-bit occupation; values 0..8 (the lower half).
    let values: Vec<u64> = (0..8).collect();
    let patterns = minimize_patterns(&values, 4);
    // Should collapse to a single pattern: bit 3 = 0, bits 0-2 free.
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0], BitPattern::new(0, 0b1000));
}

#[test]
fn worst_case_range_has_bounded_pattern_count() {
    // The "worst-case" range [1, 2^k - 2] produces 2(k-1) patterns.
    for bits in 2u8..=16 {
        let hi = (1u64 << bits) - 2;
        let ps = range_to_prefixes(1, hi, bits);
        assert!(
            ps.len() <= 2 * bits as usize,
            "bits={bits}: {} patterns exceeds 2*bits",
            ps.len()
        );
    }
}

#[test]
fn any_pattern_covers_full_domain_and_bit_pattern_matches_all() {
    let p = BitPattern::any(5);
    for v in 0..32u64 {
        assert!(p.matches(v));
    }
    assert_eq!(p.cardinality(5), 32);
}
