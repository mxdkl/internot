//! Odometer-style cross-product enumeration for find() queries.
//!
//! A [`FieldCursor`] enumerates the u64 values a single field takes on
//! during a query — either chaining through the field's BitPatterns
//! (constrained) or walking 0..2^width (unconstrained).
//!
//! [`FindIter`] (added in Task 5) composes cursors into an odometer
//! that yields W-typed IDs.

use crate::search::BitPattern;

/// Per-field cursor. If `patterns` is empty the cursor is unconstrained
/// and walks `0..2^width`. Otherwise it chains through `patterns`, yielding
/// every value of each pattern before advancing to the next pattern.
pub(crate) struct FieldCursor {
    pub(crate) width: u8,
    pub(crate) patterns: Vec<BitPattern>,
    /// For each pattern: the ascending free-bit positions within `width`.
    /// Precomputed at construction. Empty when `patterns` is empty.
    free_positions_per_pattern: Vec<Vec<u32>>,
    pattern_idx: usize,
    within_idx: u64,
}

impl FieldCursor {
    pub(crate) fn new(width: u8, patterns: Vec<BitPattern>) -> Self {
        assert!(width <= 64, "width must be ≤ 64");
        let domain_mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let free_positions_per_pattern = patterns
            .iter()
            .map(|p| {
                let free = !p.mask & domain_mask;
                (0..width as u32)
                    .filter(|bit| free & (1u64 << bit) != 0)
                    .collect()
            })
            .collect();
        FieldCursor {
            width,
            patterns,
            free_positions_per_pattern,
            pattern_idx: 0,
            within_idx: 0,
        }
    }

    /// Current field value.
    pub(crate) fn current_value(&self) -> u64 {
        if self.patterns.is_empty() {
            self.within_idx
        } else {
            let pattern = &self.patterns[self.pattern_idx];
            let free = &self.free_positions_per_pattern[self.pattern_idx];
            let mut out = pattern.fixed;
            for (k, &pos) in free.iter().enumerate() {
                if self.within_idx & (1u64 << k) != 0 {
                    out |= 1u64 << pos;
                }
            }
            out
        }
    }

    /// Advance to the next value. Returns `false` when the cursor wraps
    /// (exhausted one full cycle); the caller is expected to call
    /// `reset()` on cursors to the right of this one.
    pub(crate) fn advance(&mut self) -> bool {
        self.within_idx += 1;
        if self.patterns.is_empty() {
            let domain = if self.width == 64 {
                u64::MAX
            } else {
                1u64 << self.width
            };
            if self.within_idx >= domain {
                self.within_idx = 0;
                return false;
            }
            true
        } else {
            let free = self.free_positions_per_pattern[self.pattern_idx].len();
            let total = if free >= 64 { u64::MAX } else { 1u64 << free };
            if self.within_idx >= total {
                self.within_idx = 0;
                self.pattern_idx += 1;
                if self.pattern_idx >= self.patterns.len() {
                    self.pattern_idx = 0;
                    return false;
                }
            }
            true
        }
    }

    pub(crate) fn reset(&mut self) {
        self.pattern_idx = 0;
        self.within_idx = 0;
    }

    /// Total values this cursor will yield over one full cycle.
    pub(crate) fn cardinality(&self) -> u64 {
        if self.patterns.is_empty() {
            if self.width == 64 {
                u64::MAX
            } else {
                1u64 << self.width
            }
        } else {
            self.patterns
                .iter()
                .map(|p| p.cardinality(self.width))
                .fold(0u64, |acc, n| acc.saturating_add(n))
        }
    }
}

use crate::bits::BitLayout;
use crate::word::BitWord;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Iterator over the IDs matching a `find()` query.
///
/// Internal invariant: `cursors` and `field_offsets` are parallel vectors —
/// index `i` refers to the same field. Field names are resolved to
/// `(offset, width)` pairs at construction and never looked up by name
/// during iteration. This is the hot path: `next()` runs once per yielded
/// candidate.
pub struct FindIter<W: BitWord> {
    /// `(bit_offset_from_lsb, bit_width)` per cursor, cached from the
    /// layout at construction. Parallel to `cursors`.
    field_offsets: Vec<(u32, u32)>,
    cursors: Vec<FieldCursor>,
    done: bool,
    _word: PhantomData<W>,
}

impl<W: BitWord> FindIter<W> {
    pub(crate) fn new(
        layout: &BitLayout<W>,
        mut per_field: HashMap<String, Vec<BitPattern>>,
    ) -> Self {
        let mut field_offsets = Vec::new();
        let mut cursors = Vec::new();
        let mut total_free_bits: u32 = 0;

        for name in layout.field_names() {
            let (offset, width) = layout
                .field_offset_width(name)
                .expect("layout returned a field it doesn't know about");
            let patterns = per_field.remove(name).unwrap_or_default();
            let cursor = FieldCursor::new(width as u8, patterns);
            total_free_bits = total_free_bits.saturating_add(count_free_bits(&cursor));
            field_offsets.push((offset, width));
            cursors.push(cursor);
        }

        // Any leftover entries in `per_field` are fields the layout doesn't know.
        if !per_field.is_empty() {
            let unknown: Vec<String> = per_field.keys().cloned().collect();
            panic!("query references fields not in layout: {unknown:?}");
        }

        // Note: the underlying `PatternEnumerator` handles arbitrary
        // free-bit counts (caps `total` at u64::MAX). Consumers bound
        // consumption with `.take(N)` or `.scan_budget(N)` — standard
        // lazy-iterator idiom. The pre-flight `total_free_bits >= 64`
        // panic was removed 2026-05-04 (see
        // docs/superpowers/specs/2026-05-04-person-512bit-substrate.md).
        let _unused = total_free_bits; // kept for future telemetry / debug

        let done = cursors.iter().any(|c| c.cardinality() == 0);

        FindIter {
            field_offsets,
            cursors,
            done,
            _word: PhantomData,
        }
    }
}

fn count_free_bits(cursor: &FieldCursor) -> u32 {
    if cursor.patterns.is_empty() {
        cursor.width as u32
    } else {
        // Use max free-bit count across patterns as an upper bound. Cursors
        // walk one pattern at a time, so at any moment only the current
        // pattern's free bits contribute; using max is conservative.
        cursor
            .patterns
            .iter()
            .map(|p| p.free_bits(cursor.width))
            .max()
            .unwrap_or(0)
    }
}

impl<W: BitWord> Iterator for FindIter<W> {
    type Item = W;

    fn next(&mut self) -> Option<W> {
        if self.done {
            return None;
        }
        // Compose current W-ID directly from cached offsets — no allocation,
        // no name lookup. Profiling (docs/profiling/2026-04-23-query-hotspots.md)
        // attributed ~63% of find_filter_static cycles to the previous path
        // which allocated a Vec<(&str, u64)> per call and re-resolved field
        // names via linear string scans inside BitLayout::compose.
        let mut id = W::zero();
        for (cur, &(offset, width)) in self.cursors.iter().zip(self.field_offsets.iter()) {
            id = id.insert_bits(offset, width, cur.current_value());
        }

        // Odometer: advance the rightmost cursor; on wrap, reset it and
        // carry to the next-left cursor.
        let mut i = self.cursors.len();
        loop {
            if i == 0 {
                self.done = true;
                break;
            }
            i -= 1;
            if self.cursors[i].advance() {
                for j in (i + 1)..self.cursors.len() {
                    self.cursors[j].reset();
                }
                break;
            }
            // wrapped; carry further left
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconstrained_cursor_walks_full_domain() {
        let mut cur = FieldCursor::new(4, vec![]);
        let mut seen = Vec::new();
        seen.push(cur.current_value());
        while cur.advance() {
            seen.push(cur.current_value());
        }
        assert_eq!(seen, (0..16u64).collect::<Vec<_>>());
    }

    #[test]
    fn constrained_cursor_walks_single_exact_pattern() {
        let mut cur = FieldCursor::new(8, vec![BitPattern::exact(42, 8)]);
        assert_eq!(cur.current_value(), 42);
        assert!(!cur.advance()); // one value, then wrap
    }

    #[test]
    fn constrained_cursor_walks_multi_pattern_chain() {
        let mut cur = FieldCursor::new(4, vec![BitPattern::exact(1, 4), BitPattern::exact(7, 4)]);
        let mut seen = Vec::new();
        seen.push(cur.current_value());
        while cur.advance() {
            seen.push(cur.current_value());
        }
        assert_eq!(seen, vec![1, 7]);
    }

    #[test]
    fn constrained_cursor_walks_pattern_with_free_bits() {
        // Pattern: bit 3 pinned to 1, bits 0..2 free → values 0b1000..=0b1111
        let p = BitPattern::new(0b1000, 0b1000);
        let mut cur = FieldCursor::new(4, vec![p]);
        let mut seen = Vec::new();
        seen.push(cur.current_value());
        while cur.advance() {
            seen.push(cur.current_value());
        }
        seen.sort();
        assert_eq!(seen, vec![8, 9, 10, 11, 12, 13, 14, 15]);
    }

    #[test]
    fn advance_returns_false_on_wrap() {
        let mut cur = FieldCursor::new(2, vec![]);
        assert_eq!(cur.current_value(), 0);
        assert!(cur.advance()); // → 1
        assert!(cur.advance()); // → 2
        assert!(cur.advance()); // → 3
        assert!(!cur.advance()); // wrap → 0
        assert_eq!(cur.current_value(), 0);
    }

    #[test]
    fn reset_restores_initial_state() {
        let mut cur = FieldCursor::new(4, vec![]);
        cur.advance();
        cur.advance();
        cur.advance();
        assert_ne!(cur.current_value(), 0);
        cur.reset();
        assert_eq!(cur.current_value(), 0);
    }

    #[test]
    fn cardinality_matches_unconstrained_domain() {
        let cur = FieldCursor::new(4, vec![]);
        assert_eq!(cur.cardinality(), 16);
    }

    #[test]
    fn cardinality_sums_constrained_patterns() {
        let cur = FieldCursor::new(4, vec![BitPattern::exact(1, 4), BitPattern::any(4)]);
        // exact covers 1, any covers 16 → 17
        assert_eq!(cur.cardinality(), 17);
    }

    use crate::bits::BitLayout;

    fn u64_layout() -> BitLayout<u64> {
        BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap()
    }

    #[test]
    fn find_iter_single_pinned_field_enumerates_others() {
        // Pin a = 5; b is unconstrained → 16 results, all with a = 5.
        let layout = u64_layout();
        let mut per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
        per_field.insert("a".to_string(), vec![BitPattern::exact(5, 4)]);
        let ids: Vec<u64> = FindIter::new(&layout, per_field).collect();
        assert_eq!(ids.len(), 16);
        for id in &ids {
            assert_eq!(layout.extract(*id, "a"), 5);
        }
        let b_values: Vec<u64> = ids.iter().map(|id| layout.extract(*id, "b")).collect();
        let mut sorted = b_values.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn find_iter_two_pinned_fields_yields_single_id() {
        let layout = u64_layout();
        let mut per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
        per_field.insert("a".to_string(), vec![BitPattern::exact(3, 4)]);
        per_field.insert("b".to_string(), vec![BitPattern::exact(7, 4)]);
        let ids: Vec<u64> = FindIter::new(&layout, per_field).collect();
        assert_eq!(ids.len(), 1);
        let id = ids[0];
        assert_eq!(layout.extract(id, "a"), 3);
        assert_eq!(layout.extract(id, "b"), 7);
    }

    #[test]
    fn find_iter_full_unconstrained_iterates_lazily() {
        // Pre-2026-05 this panicked. Now find() is lazy — consumers
        // bound consumption with .take(N) or scan_budget. Iterator
        // construction is O(1) regardless of free bit count.
        let layout = BitLayout::<u64>::new(vec![("x", 64)]).unwrap();
        let per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
        let iter = FindIter::new(&layout, per_field);
        // Take a small bounded slice; should not OOM or hang.
        let ids: Vec<u64> = iter.take(10).collect();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn find_iter_is_deterministic() {
        let layout = u64_layout();
        let build_iter = || {
            let mut per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
            per_field.insert("a".to_string(), vec![BitPattern::exact(2, 4)]);
            FindIter::new(&layout, per_field)
        };
        let ids1: Vec<u64> = build_iter().collect();
        let ids2: Vec<u64> = build_iter().collect();
        assert_eq!(ids1, ids2);
    }

    #[test]
    fn find_iter_with_64_plus_free_bits_iterates_lazily() {
        use crate::space::Space;
        use crate::word::{BitWord, U512};
        use crate::world::World;

        // Layout total = 200 bits (well over the previous 64-bit ceiling).
        // Split fields into <=64-bit chunks (extract_bits returns u64).
        let layout = BitLayout::<U512>::new(vec![
            ("a", 12),
            ("b", 8),
            ("c", 6),
            ("d", 6),
            ("rest_a", 64),
            ("rest_b", 64),
            ("rest_c", 40),
        ])
        .unwrap();
        let mut space = Space::<U512>::new("big", layout);
        space
            .indexable_attribute::<u32, _>("a", "a", |v| v as u32)
            .unwrap();
        let mut world = World::<U512>::new();
        world.register(space).unwrap();
        let s = world.space("big").unwrap();

        // Pin only `a` (12 bits); 188 bits remain free. Should NOT panic
        // after Task 3's removal of the pre-flight ceiling.
        let results: Vec<U512> = s
            .find()
            .where_eq("a", 5)
            .scan_budget(8)
            .execute()
            .into_iter()
            .take(8)
            .collect();

        assert_eq!(
            results.len(),
            8,
            "should yield exactly 8 ids despite huge free space"
        );
        for id in &results {
            assert_eq!(id.extract_bits(0, 12), 5, "field `a` mismatch");
        }
    }
}
