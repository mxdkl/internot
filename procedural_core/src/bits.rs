//! Bit layout utilities for structured ID words of any fixed width.

use crate::word::BitWord;
use std::collections::HashSet;
use std::marker::PhantomData;

/// Declares how the bits of an entity ID are partitioned into named fields.
///
/// Field widths must sum to at most `W::BITS`. Field names must be unique.
/// Zero-width fields are rejected. Once constructed, a `BitLayout` is immutable
/// and provides `extract` and `compose` for moving between named fields and raw
/// `W` values.
///
/// # Field order matters for `find()`
///
/// Fields are laid out LSB-first in declaration order: the first field
/// occupies the lowest bits, the last field the highest. `find()` enumerates
/// IDs in odometer order, where the *last-declared* field varies fastest.
/// Put fields you expect to constrain narrowly (via `where_eq`) **last** so
/// a bounded query returns varied results before exhausting the unconstrained
/// low-order bits (e.g. `entropy`). See `find()` for details.
#[derive(Debug, Clone)]
pub struct BitLayout<W: BitWord> {
    fields: Vec<(String, u8)>,  // name -> width, in declared order
    offsets: Vec<(String, u8)>, // name -> offset from LSB, cached
    total: u32,
    _word: PhantomData<W>,
}

#[derive(Debug, Clone)]
pub enum BitLayoutError {
    Overflow(u32),
    DuplicateField(String),
    ZeroWidth(String),
}

impl std::fmt::Display for BitLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitLayoutError::Overflow(n) => write!(f, "total bit width {} exceeds word size", n),
            BitLayoutError::DuplicateField(s) => write!(f, "duplicate field name: {}", s),
            BitLayoutError::ZeroWidth(s) => write!(f, "field {} has zero width", s),
        }
    }
}

impl std::error::Error for BitLayoutError {}

impl<W: BitWord> BitLayout<W> {
    pub fn new(fields: Vec<(&str, u8)>) -> Result<Self, BitLayoutError> {
        let mut seen = HashSet::new();
        let mut total = 0u32;
        for (name, width) in &fields {
            if *width == 0 {
                return Err(BitLayoutError::ZeroWidth((*name).to_string()));
            }
            if !seen.insert(*name) {
                return Err(BitLayoutError::DuplicateField((*name).to_string()));
            }
            total += *width as u32;
        }
        if total > W::BITS {
            return Err(BitLayoutError::Overflow(total));
        }

        let mut offsets = Vec::with_capacity(fields.len());
        let mut cumulative = 0u8;
        for (name, width) in &fields {
            offsets.push(((*name).to_string(), cumulative));
            cumulative += *width;
        }

        Ok(BitLayout {
            fields: fields.iter().map(|(n, w)| ((*n).to_string(), *w)).collect(),
            offsets,
            total,
            _word: PhantomData,
        })
    }

    pub fn total_width(&self) -> u32 {
        self.total
    }

    /// Extract the value of a named field from an id word.
    /// Panics if the field does not exist in this layout.
    pub fn extract(&self, id: W, field: &str) -> u64 {
        let (offset, width) = self
            .field_info(field)
            .unwrap_or_else(|| panic!("unknown field: {:?}", field));
        id.extract_bits(offset as u32, width as u32)
    }

    /// Compose an id word from field values.
    ///
    /// Release builds mask values to the field's declared width (low
    /// bits only). **Debug builds assert** that the value fits — a
    /// `compose("industry_idx", 1024)` against a 6-bit field is almost
    /// always a caller bug, not intentional truncation.
    /// Unlisted fields default to zero. Panics on unknown field name.
    pub fn compose(&self, values: &[(&str, u64)]) -> W {
        let mut result = W::zero();
        for (name, value) in values {
            let (offset, width) = self
                .field_info(name)
                .unwrap_or_else(|| panic!("unknown field: {:?}", name));
            debug_assert!(
                width as u32 == 64 || *value < (1u64 << width),
                "value {} overflows field {:?} of width {} (max {}) — caller bug",
                value,
                name,
                width,
                (1u64 << width).saturating_sub(1)
            );
            result = result.insert_bits(offset as u32, width as u32, *value);
        }
        result
    }

    /// Return true if `name` is a declared field in this layout.
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|(n, _)| n == name)
    }

    /// Iterate declared field names in the order they were defined.
    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|(n, _)| n.as_str())
    }

    /// Return `(offset_from_lsb, width)` for the named field, or `None` if not found.
    pub fn field_offset_width(&self, name: &str) -> Option<(u32, u32)> {
        self.field_info(name)
            .map(|(offset, width)| (offset as u32, width as u32))
    }

    /// Return a fast closure that extracts the value of `field` from an id.
    ///
    /// Resolves the field name once; the returned closure does the bit
    /// extract with no further name lookup. `Copy`, `Send`, and `Sync`, so
    /// it can be cloned into trajectory closures, samplers, or anywhere
    /// a cheap `Fn(W) -> u64` is useful.
    ///
    /// Panics if `field` is not in the layout — catches the error at
    /// setup time rather than deep inside a per-candidate call.
    ///
    /// ```
    /// use procedural_core::bits::BitLayout;
    /// let layout = BitLayout::<u64>::new(vec![("age", 7), ("name_idx", 6)]).unwrap();
    /// let get_age = layout.extractor("age");
    /// // In a closure that can't capture &layout:
    /// let age_predicate = move |id: u64| get_age(id) > 18;
    /// assert!(age_predicate(25u64));
    /// ```
    pub fn extractor(&self, field: &str) -> impl Fn(W) -> u64 + Copy + Send + Sync {
        let (offset, width) = self
            .field_offset_width(field)
            .unwrap_or_else(|| panic!("unknown field: {field:?}"));
        move |id: W| id.extract_bits(offset, width)
    }

    fn field_info(&self, name: &str) -> Option<(u8, u8)> {
        let offset = self.offsets.iter().find(|(n, _)| n == name)?.1;
        let width = self.fields.iter().find(|(n, _)| n == name)?.1;
        Some((offset, width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_layout_constructs_when_widths_sum_to_64_or_less() {
        let layout = BitLayout::<u64>::new(vec![
            ("gen", 4),
            ("family", 28),
            ("sibling", 4),
            ("entropy", 28),
        ])
        .expect("should construct");
        assert_eq!(layout.total_width(), 64);
    }

    #[test]
    fn bit_layout_accepts_unused_bits() {
        let layout =
            BitLayout::<u64>::new(vec![("gen", 4), ("family", 16)]).expect("should construct");
        assert_eq!(layout.total_width(), 20);
    }

    #[test]
    fn bit_layout_rejects_overflow() {
        let err =
            BitLayout::<u64>::new(vec![("a", 40), ("b", 40)]).expect_err("should reject overflow");
        assert!(matches!(err, BitLayoutError::Overflow(80)));
    }

    #[test]
    fn bit_layout_rejects_duplicate_fields() {
        let err = BitLayout::<u64>::new(vec![("gen", 4), ("gen", 4)])
            .expect_err("should reject duplicates");
        assert!(matches!(err, BitLayoutError::DuplicateField(_)));
    }

    #[test]
    fn bit_layout_rejects_zero_width() {
        let err =
            BitLayout::<u64>::new(vec![("gen", 0)]).expect_err("should reject zero-width fields");
        assert!(matches!(err, BitLayoutError::ZeroWidth(_)));
    }

    fn sample_layout() -> BitLayout<u64> {
        BitLayout::new(vec![
            ("gen", 4),
            ("family", 28),
            ("sibling", 4),
            ("entropy", 28),
        ])
        .unwrap()
    }

    #[test]
    fn extract_reads_field_values() {
        let layout = sample_layout();
        // bit layout (offsets from LSB): gen:0..4, family:4..32, sibling:32..36, entropy:36..64
        // compose: gen=0xF, family=0xFFFFFFF, sibling=0xF, entropy=0xFFFFFFF
        let id: u64 = (0x0FFFFFFFu64 << 36) | (0xFu64 << 32) | (0x0FFFFFFFu64 << 4) | 0xFu64;
        assert_eq!(layout.extract(id, "gen"), 0xF);
        assert_eq!(layout.extract(id, "family"), 0x0FFFFFFF);
        assert_eq!(layout.extract(id, "sibling"), 0xF);
        assert_eq!(layout.extract(id, "entropy"), 0x0FFFFFFF);
    }

    #[test]
    fn compose_sets_field_values() {
        let layout = sample_layout();
        let id = layout.compose(&[
            ("gen", 3),
            ("family", 0x1234567),
            ("sibling", 7),
            ("entropy", 0x89ABCDE),
        ]);
        assert_eq!(layout.extract(id, "gen"), 3);
        assert_eq!(layout.extract(id, "family"), 0x1234567);
        assert_eq!(layout.extract(id, "sibling"), 7);
        assert_eq!(layout.extract(id, "entropy"), 0x89ABCDE);
    }

    #[test]
    fn compose_packs_max_in_range_value() {
        // Values exactly at width-1 (the largest legal value) compose
        // and round-trip cleanly.
        let layout = BitLayout::<u64>::new(vec![("small", 4), ("big", 32)]).unwrap();
        let id = layout.compose(&[("small", 0xF), ("big", 0xFFFFFFFF)]);
        assert_eq!(layout.extract(id, "small"), 0xF);
        assert_eq!(layout.extract(id, "big"), 0xFFFFFFFF);
    }

    #[cfg(debug_assertions)] // the check is a debug_assert!, compiled out in release
    #[test]
    #[should_panic(expected = "overflows field")]
    fn compose_panics_in_debug_when_value_exceeds_field_width() {
        // Pre-fix: this silently truncated 0xFF to 0xF on a 4-bit
        // field — a real footgun. Now it asserts in debug builds.
        let layout = BitLayout::<u64>::new(vec![("small", 4)]).unwrap();
        let _ = layout.compose(&[("small", 0xFF)]);
    }

    #[test]
    fn round_trip_preserves_values_within_widths() {
        use proptest::prelude::*;
        let layout = sample_layout();
        proptest!(|(gen in 0u64..16, family in 0u64..(1 << 28), sibling in 0u64..16, entropy in 0u64..(1 << 28))| {
            let id = layout.compose(&[
                ("gen", gen),
                ("family", family),
                ("sibling", sibling),
                ("entropy", entropy),
            ]);
            prop_assert_eq!(layout.extract(id, "gen"), gen);
            prop_assert_eq!(layout.extract(id, "family"), family);
            prop_assert_eq!(layout.extract(id, "sibling"), sibling);
            prop_assert_eq!(layout.extract(id, "entropy"), entropy);
        });
    }

    #[test]
    #[should_panic(expected = "unknown field")]
    fn extract_panics_on_unknown_field() {
        let layout = sample_layout();
        layout.extract(0, "nope");
    }

    #[test]
    fn u128_layout_round_trip() {
        let layout = BitLayout::<u128>::new(vec![
            ("gen", 8),
            ("family", 56),
            ("sibling", 8),
            ("extras", 56), // total = 128
        ])
        .expect("should construct");
        assert_eq!(layout.total_width(), 128);

        let id: u128 = layout.compose(&[
            ("gen", 0xFF),
            ("family", 0x00FF_FFFF_FFFF_FFFF),
            ("sibling", 0x3C),
            ("extras", 0x0012_3456_789A_BCDE),
        ]);

        assert_eq!(layout.extract(id, "gen"), 0xFF);
        assert_eq!(layout.extract(id, "family"), 0x00FF_FFFF_FFFF_FFFF);
        assert_eq!(layout.extract(id, "sibling"), 0x3C);
        assert_eq!(layout.extract(id, "extras"), 0x0012_3456_789A_BCDE);
    }

    #[test]
    fn u128_layout_full_width_overflow() {
        let err = BitLayout::<u128>::new(vec![
            ("a", 64),
            ("b", 64),
            ("c", 8), // 136 > 128
        ])
        .expect_err("should overflow");
        assert!(matches!(err, BitLayoutError::Overflow(136)));
    }

    #[test]
    fn u64_layout_over_64_rejected() {
        // A u64 layout must not accept >64 total
        let err = BitLayout::<u64>::new(vec![
            ("a", 64),
            ("b", 8), // 72 > 64
        ])
        .expect_err("should overflow");
        assert!(matches!(err, BitLayoutError::Overflow(72)));
    }

    #[test]
    fn field_names_iterates_in_declared_order() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 8), ("c", 16)]).unwrap();
        let names: Vec<&str> = layout.field_names().collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn field_offset_width_is_public() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 8)]).unwrap();
        assert_eq!(layout.field_offset_width("a"), Some((0, 4)));
        assert_eq!(layout.field_offset_width("b"), Some((4, 8)));
        assert_eq!(layout.field_offset_width("nope"), None);
    }

    #[test]
    fn extractor_matches_extract_for_all_fields() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 8), ("c", 20)]).unwrap();
        let id = layout.compose(&[("a", 0xA), ("b", 0xBC), ("c", 0x12345)]);
        let get_a = layout.extractor("a");
        let get_b = layout.extractor("b");
        let get_c = layout.extractor("c");
        assert_eq!(get_a(id), layout.extract(id, "a"));
        assert_eq!(get_b(id), layout.extract(id, "b"));
        assert_eq!(get_c(id), layout.extract(id, "c"));
        assert_eq!(get_a(id), 0xA);
        assert_eq!(get_b(id), 0xBC);
        assert_eq!(get_c(id), 0x12345);
    }

    #[test]
    fn extractor_is_copy_and_independent_of_layout_lifetime() {
        let get_age = {
            let layout = BitLayout::<u64>::new(vec![("age", 7)]).unwrap();
            layout.extractor("age")
            // `layout` dropped here — extractor must still work.
        };
        let copy = get_age; // verifies Copy
        let id = 0b0010101u64; // age = 21
        assert_eq!(get_age(id), 21);
        assert_eq!(copy(id), 21);
    }

    #[test]
    #[should_panic(expected = "unknown field")]
    fn extractor_panics_on_unknown_field() {
        let layout = BitLayout::<u64>::new(vec![("a", 4)]).unwrap();
        let _ = layout.extractor("nope");
    }

    #[test]
    fn extractor_works_with_u128() {
        let layout = BitLayout::<u128>::new(vec![("lo", 64), ("hi", 64)]).expect("u128 layout");
        let id = layout.compose(&[("lo", 0xAAAA_BBBB), ("hi", 0xCCCC_DDDD)]);
        let get_lo = layout.extractor("lo");
        let get_hi = layout.extractor("hi");
        assert_eq!(get_lo(id), 0xAAAA_BBBB);
        assert_eq!(get_hi(id), 0xCCCC_DDDD);
    }
}
