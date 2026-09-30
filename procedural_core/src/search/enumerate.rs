//! PatternEnumerator — iterate the IDs matching a BitPattern.

use crate::search::BitPattern;

/// Iterator over the IDs matching a [`BitPattern`] within a fixed-width domain.
///
/// For `k` free bits, yields exactly `2^k` IDs. The walk is deterministic:
/// the n-th emitted ID deposits the bits of `n` into the pattern's free
/// positions (ascending position order).
pub struct PatternEnumerator {
    fixed: u64,
    free_positions: Vec<u32>,
    total: u64,
    index: u64,
}

impl PatternEnumerator {
    /// Construct an enumerator for `pattern` over the `width`-bit domain.
    pub(crate) fn new(pattern: BitPattern, width: u8) -> Self {
        assert!(width <= 64, "width must be ≤ 64");
        let domain_mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let free_mask = !pattern.mask & domain_mask;
        let mut free_positions = Vec::with_capacity(free_mask.count_ones() as usize);
        for bit in 0..width as u32 {
            if free_mask & (1u64 << bit) != 0 {
                free_positions.push(bit);
            }
        }
        let total = if free_positions.len() >= 64 {
            u64::MAX
        } else {
            1u64 << free_positions.len()
        };
        PatternEnumerator {
            fixed: pattern.fixed,
            free_positions,
            total,
            index: 0,
        }
    }

    /// Total IDs this enumerator will yield if exhausted. Saturates to
    /// `u64::MAX` for patterns with ≥ 64 free bits.
    pub fn cardinality(&self) -> u64 {
        self.total
    }
}

impl Iterator for PatternEnumerator {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        if self.index >= self.total {
            return None;
        }
        let mut out = self.fixed;
        for (k, &pos) in self.free_positions.iter().enumerate() {
            if self.index & (1u64 << k) != 0 {
                out |= 1u64 << pos;
            }
        }
        self.index += 1;
        Some(out)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.total.saturating_sub(self.index);
        match usize::try_from(remaining) {
            Ok(n) => (n, Some(n)),
            Err(_) => (usize::MAX, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pattern_yields_single_value() {
        let ids: Vec<u64> = PatternEnumerator::new(BitPattern::exact(5, 4), 4).collect();
        assert_eq!(ids, vec![5]);
    }

    #[test]
    fn any_pattern_yields_full_domain() {
        let ids: Vec<u64> = PatternEnumerator::new(BitPattern::any(3), 3).collect();
        assert_eq!(ids, (0..8u64).collect::<Vec<_>>());
    }

    #[test]
    fn two_free_bits_yields_four_values() {
        // bit 3 = 1, bit 2 = 0, bits 1..0 free
        let p = BitPattern::new(0b1000, 0b1100);
        let ids: Vec<u64> = PatternEnumerator::new(p, 4).collect();
        assert_eq!(ids, vec![0b1000, 0b1001, 0b1010, 0b1011]);
    }

    #[test]
    fn cardinality_matches_yielded_count() {
        let p = BitPattern::new(0b1000, 0b1100); // 2 free bits
        let enumerator = PatternEnumerator::new(p, 4);
        assert_eq!(enumerator.cardinality(), 4);
        let ids: Vec<u64> = enumerator.collect();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn size_hint_is_exact_for_small_domains() {
        let p = BitPattern::any(4);
        let mut it = PatternEnumerator::new(p, 4);
        assert_eq!(it.size_hint(), (16, Some(16)));
        it.next();
        assert_eq!(it.size_hint(), (15, Some(15)));
    }

    #[test]
    fn width_0_pattern_yields_single_zero() {
        let ids: Vec<u64> = PatternEnumerator::new(BitPattern::any(0), 0).collect();
        assert_eq!(ids, vec![0]);
    }

    #[test]
    fn no_duplicates_in_output() {
        let p = BitPattern::new(0b0100, 0b0101); // bits 0, 2 fixed; bits 1, 3 free
        let ids: Vec<u64> = PatternEnumerator::new(p, 4).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "found duplicates: {:?}", ids);
    }

    #[test]
    fn all_yielded_values_match_pattern() {
        let p = BitPattern::new(0b0100, 0b0101);
        for v in PatternEnumerator::new(p, 4) {
            assert!(p.matches(v), "v={v:04b} doesn't match pattern");
        }
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn enumeration_matches_pattern(
            width in 1u8..=8u8,
            raw_fixed in any::<u64>(),
            raw_mask in any::<u64>(),
        ) {
            let domain_mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
            let mask = raw_mask & domain_mask;
            let pattern = BitPattern::new(raw_fixed & domain_mask, mask);

            let enumerator = PatternEnumerator::new(pattern, width);
            let cardinality = enumerator.cardinality();
            let ids: Vec<u64> = enumerator.collect();

            // Count matches cardinality.
            prop_assert_eq!(ids.len() as u64, cardinality, "count != cardinality");

            // Cardinality is 2^free_bits.
            let free = pattern.free_bits(width) as u64;
            let expected = 1u64 << free;
            prop_assert_eq!(cardinality, expected, "cardinality != 2^free_bits");

            // Every yielded value matches the pattern and is in-domain.
            for &v in &ids {
                prop_assert!(pattern.matches(v), "v={} doesn't match", v);
                prop_assert!(v <= domain_mask, "v={} outside domain", v);
            }

            // No duplicates.
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            sorted.dedup();
            prop_assert_eq!(sorted.len(), ids.len(), "duplicates found");
        }
    }
}
