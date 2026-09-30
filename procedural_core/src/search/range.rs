//! Decompose closed integer ranges into minimal bit-pattern covers.

use crate::search::BitPattern;

/// Decompose the closed range `[lo, hi]` on a `bits`-wide unsigned field
/// into the minimal set of `BitPattern`s whose union is exactly `[lo, hi]`.
///
/// # Panics
/// * if `bits > 64`
/// * if `lo > hi`
/// * if `bits < 64` and `hi >= 1 << bits`
pub fn range_to_prefixes(lo: u64, hi: u64, bits: u8) -> Vec<BitPattern> {
    assert!(bits <= 64, "bits must be ≤ 64");
    assert!(lo <= hi, "lo must be ≤ hi");
    let field_mask: u64 = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    if bits < 64 {
        assert!(hi <= field_mask, "hi must fit in the {}-bit domain", bits);
    }

    // Special case: full 64-bit domain. One all-free pattern.
    if bits == 64 && lo == 0 && hi == u64::MAX {
        return vec![BitPattern::new(0, 0)];
    }

    let mut out = Vec::new();
    let mut x = lo;
    loop {
        // Alignment: largest power-of-2 block that starts at x.
        let tz = if x == 0 {
            bits as u32
        } else {
            x.trailing_zeros().min(bits as u32)
        };
        // Room: floor_log2(hi - x + 1). Safe because we handled full-range above.
        let room = hi - x + 1;
        let room_log2 = 63 - room.leading_zeros();
        let size_log2 = tz.min(room_log2);
        let size = 1u64 << size_log2;
        let low_mask = size - 1;
        let mask = field_mask & !low_mask;
        out.push(BitPattern::new(x, mask));

        // Advance, stopping at hi or on overflow.
        match x.checked_add(size) {
            Some(next) if next <= hi => x = next,
            _ => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_range_yields_one_full_mask_pattern() {
        let ps = range_to_prefixes(5, 5, 4);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::exact(5, 4));
    }

    #[test]
    fn full_aligned_range_yields_one_no_mask_pattern() {
        // [0, 15] on 4 bits: one pattern covering everything.
        let ps = range_to_prefixes(0, 15, 4);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::any(4));
    }

    #[test]
    fn worst_case_3_bit_emits_2k_minus_2_patterns() {
        // [1, 6] on 3 bits: worst case, 2(3-1) = 4 patterns.
        let ps = range_to_prefixes(1, 6, 3);
        assert_eq!(ps.len(), 4);
        // Verify each integer in [1,6] matches exactly one pattern.
        for v in 1..=6u64 {
            let c = ps.iter().filter(|p| p.matches(v)).count();
            assert_eq!(c, 1, "value {v} matched {c} patterns");
        }
        // And 0 and 7 match zero patterns.
        assert_eq!(ps.iter().filter(|p| p.matches(0)).count(), 0);
        assert_eq!(ps.iter().filter(|p| p.matches(7)).count(), 0);
    }

    #[test]
    fn worst_case_4_bit_emits_6_patterns() {
        // [1, 14] on 4 bits: 2(4-1) = 6.
        let ps = range_to_prefixes(1, 14, 4);
        assert_eq!(ps.len(), 6);
    }

    #[test]
    fn worst_case_8_bit_emits_14_patterns() {
        // [1, 254] on 8 bits: 2(8-1) = 14.
        let ps = range_to_prefixes(1, 254, 8);
        assert_eq!(ps.len(), 14);
    }

    #[test]
    fn cross_block_range_covers_exactly() {
        // [3, 12] on 4 bits.
        let ps = range_to_prefixes(3, 12, 4);
        for v in 0..16u64 {
            let c = ps.iter().filter(|p| p.matches(v)).count();
            let expected = if (3..=12).contains(&v) { 1 } else { 0 };
            assert_eq!(c, expected, "v={v}: matched {c}, expected {expected}");
        }
    }

    #[test]
    fn full_64_bit_range_is_single_any_pattern() {
        let ps = range_to_prefixes(0, u64::MAX, 64);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::new(0, 0));
    }

    #[test]
    #[should_panic(expected = "lo must be ≤ hi")]
    fn panics_on_inverted_bounds() {
        let _ = range_to_prefixes(5, 3, 4);
    }

    #[test]
    #[should_panic(expected = "hi must fit")]
    fn panics_on_hi_outside_domain() {
        let _ = range_to_prefixes(0, 16, 4);
    }

    #[test]
    #[should_panic(expected = "bits must be ≤ 64")]
    fn panics_on_width_65() {
        let _ = range_to_prefixes(0, 0, 65);
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            max_global_rejects: 10000,
            ..ProptestConfig::default()
        })]

        #[test]
        fn coverage_is_exact_for_random_ranges(
            bits in 1u8..=10u8,
            (lo, hi) in (0u64..1024).prop_flat_map(|lo| (Just(lo), lo..1024)),
        ) {
            let domain = 1u64 << bits;
            prop_assume!(hi < domain);
            let ps = range_to_prefixes(lo, hi, bits);
            // Every value in [lo, hi] matches exactly one pattern.
            for v in lo..=hi {
                let c = ps.iter().filter(|p| p.matches(v)).count();
                prop_assert_eq!(c, 1, "v={} matched {} patterns, expected 1", v, c);
            }
            // No value outside matches any pattern.
            for v in 0..domain {
                if v < lo || v > hi {
                    let c = ps.iter().filter(|p| p.matches(v)).count();
                    prop_assert_eq!(c, 0, "v={} matched {} patterns, expected 0", v, c);
                }
            }
            // Pattern count is bounded by 2*bits.
            prop_assert!(
                ps.len() <= 2 * bits as usize,
                "emitted {} patterns for {}-bit range, expected ≤ {}",
                ps.len(), bits, 2 * bits
            );
        }
    }
}
