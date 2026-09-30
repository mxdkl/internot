//! BitPattern — a set of IDs described by pinned and free bits.

/// A set of IDs over a fixed-width field: `mask` selects which bits are
/// pinned, and `fixed` holds their required values. Any bit outside `mask`
/// is "don't care."
///
/// Invariants (enforced by constructors):
/// - `fixed & !mask == 0` — no stray bits in free positions
/// - `mask` fits in the field width the pattern was constructed against
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitPattern {
    pub fixed: u64,
    pub mask: u64,
}

impl BitPattern {
    /// Construct from raw `(fixed, mask)` — caller guarantees invariants.
    /// Free bits in `fixed` are cleared automatically.
    pub fn new(fixed: u64, mask: u64) -> Self {
        BitPattern {
            fixed: fixed & mask,
            mask,
        }
    }

    /// Pattern that matches exactly one value across all `width` bits.
    /// Panics if `width > 64` or `value` has bits set outside `width`.
    pub fn exact(value: u64, width: u8) -> Self {
        assert!(width <= 64, "width must be ≤ 64");
        let field_mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        assert!(
            value & !field_mask == 0,
            "value has bits outside the {}-bit domain",
            width
        );
        BitPattern {
            fixed: value,
            mask: field_mask,
        }
    }

    /// Trivial pattern matching every value in the `width`-bit domain.
    pub fn any(width: u8) -> Self {
        assert!(width <= 64, "width must be ≤ 64");
        BitPattern { fixed: 0, mask: 0 }
    }

    /// True when `candidate` satisfies this pattern.
    pub fn matches(&self, candidate: u64) -> bool {
        (candidate & self.mask) == self.fixed
    }

    /// Number of free bits in the `width`-bit domain.
    pub fn free_bits(&self, width: u8) -> u32 {
        assert!(width <= 64);
        let field_mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        (!self.mask & field_mask).count_ones()
    }

    /// Number of IDs covered by this pattern within the `width`-bit domain.
    pub fn cardinality(&self, width: u8) -> u64 {
        let f = self.free_bits(width);
        if f >= 64 {
            u64::MAX
        } else {
            1u64 << f
        }
    }

    /// Enumerate all `u64` values in the `width`-bit domain matching this
    /// pattern. The iterator yields exactly `2^free_bits(width)` items.
    pub fn enumerate(&self, width: u8) -> crate::search::PatternEnumerator {
        crate::search::PatternEnumerator::new(*self, width)
    }

    /// Generate a single deterministic member of the pattern's set.
    /// Pinned bits are kept; free bits are filled by hashing `seed`
    /// into the free-bit positions. Different seeds yield diverse
    /// members spread across the candidate space (unlike `enumerate()`
    /// whose first-N items cluster in the lowest free bits).
    ///
    /// `width` must match the layout width that produced this pattern.
    /// Cost is O(free_bits).
    pub fn sample(&self, seed: u64, width: u8) -> u64 {
        assert!(width <= 64, "width must be ≤ 64");
        let domain_mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let free_mask = !self.mask & domain_mask;
        let mut id = self.fixed;
        let mut k: u32 = 0;
        for bit in 0..width as u32 {
            if free_mask & (1u64 << bit) != 0 {
                let h = crate::hash::hash_int(seed, &format!("bit_sample_{k}_v1"), 2);
                id |= (h & 1) << bit;
                k += 1;
            }
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matches_only_given_value() {
        let p = BitPattern::exact(42, 8);
        assert!(p.matches(42));
        assert!(!p.matches(43));
        assert!(!p.matches(0));
    }

    #[test]
    fn any_matches_everything_in_domain() {
        let p = BitPattern::any(4);
        for v in 0..16u64 {
            assert!(p.matches(v));
        }
    }

    #[test]
    fn new_clears_free_bits_from_fixed() {
        // fixed = 0b1111, mask = 0b1100 → fixed becomes 0b1100
        let p = BitPattern::new(0b1111, 0b1100);
        assert_eq!(p.fixed, 0b1100);
        assert_eq!(p.mask, 0b1100);
    }

    #[test]
    fn free_bits_and_cardinality() {
        // 3-bit field, mask = 0b101 → 1 free bit (bit 1)
        let p = BitPattern::new(0b101, 0b101);
        assert_eq!(p.free_bits(3), 1);
        assert_eq!(p.cardinality(3), 2);
    }

    #[test]
    fn any_has_full_cardinality() {
        assert_eq!(BitPattern::any(4).cardinality(4), 16);
        assert_eq!(BitPattern::any(0).cardinality(0), 1);
    }

    #[test]
    fn matches_is_consistent_with_new() {
        let p = BitPattern::new(0b1111, 0b1100);
        // bits 2,3 pinned to 1; bits 0,1 free. Matches: 0b1100..=0b1111.
        assert!(p.matches(0b1100));
        assert!(p.matches(0b1101));
        assert!(p.matches(0b1110));
        assert!(p.matches(0b1111));
        assert!(!p.matches(0b1011));
    }

    #[test]
    #[should_panic(expected = "value has bits outside")]
    fn exact_panics_on_out_of_domain_value() {
        let _ = BitPattern::exact(16, 4);
    }

    #[test]
    #[should_panic(expected = "width must be ≤ 64")]
    fn exact_panics_on_width_65() {
        let _ = BitPattern::exact(0, 65);
    }

    #[test]
    fn enumerate_method_yields_expected_values() {
        let p = BitPattern::exact(7, 4);
        let ids: Vec<u64> = p.enumerate(4).collect();
        assert_eq!(ids, vec![7]);

        let p = BitPattern::any(3);
        let ids: Vec<u64> = p.enumerate(3).collect();
        assert_eq!(ids, (0..8u64).collect::<Vec<_>>());
    }

    // -- sample(seed) tests ---------------------------------------------------

    #[test]
    fn sample_yields_pinned_bits_unchanged_for_any_seed() {
        // Pattern: bit 3 = 1, bit 2 = 0, bits 1..0 free.
        let p = BitPattern::new(0b1000, 0b1100);
        for seed in 0..100u64 {
            let id = p.sample(seed, 4);
            assert_eq!(id & 0b1100, 0b1000, "pinned bits mutated for seed {seed}");
        }
    }

    #[test]
    fn sample_varies_free_bits_with_seed() {
        // Pattern: 16 bits with bottom 8 free.
        let p = BitPattern::new(0xAB00, 0xFF00);
        let s0 = p.sample(0, 16);
        let s1 = p.sample(1, 16);
        let s99 = p.sample(99, 16);
        assert_eq!(s0 & 0xFF00, 0xAB00, "pinned bits");
        assert_eq!(s1 & 0xFF00, 0xAB00, "pinned bits");
        assert_eq!(s99 & 0xFF00, 0xAB00, "pinned bits");
        assert!(
            s0 != s1 || s0 != s99,
            "samples didn't vary — collision or bug"
        );
    }

    #[test]
    fn sample_deterministic_for_same_seed() {
        let p = BitPattern::new(0xAB00, 0xFF00);
        assert_eq!(p.sample(42, 16), p.sample(42, 16));
    }

    #[test]
    fn sample_exact_pattern_returns_pinned_value() {
        // No free bits → every sample returns the exact value.
        let p = BitPattern::exact(0xCAFE, 16);
        for seed in 0..16u64 {
            assert_eq!(p.sample(seed, 16), 0xCAFE);
        }
    }
}
