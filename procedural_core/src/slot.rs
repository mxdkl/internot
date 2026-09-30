//! `SlotLayout` — the canonical "slot service" pattern.
//!
//! Many services in the internot stack are shaped the same way: every
//! `(owner_mail_id, idx)` is a slot in a per-owner namespace, and the
//! slot is "populated" iff `idx < count_of(owner)`. The bit layout for
//! such a service is always `[idx (LSB) | owner_mail_id]` plus reserved
//! padding for the `procedural_overlay::Session` sentinel. Enumeration
//! is direct compose, never `Space::find()` (since pinning only the
//! owner cohort would leave > 64 free bits and panic the framework).
//!
//! This module captures the *layout mechanics* — the parts every slot
//! service does identically. The genuinely domain-specific bits
//! (count distribution, hash-derived attributes, AVM struct) stay in
//! the consumer crate.
//!
//! # Example
//!
//! ```ignore
//! use procedural_core::slot::SlotLayout;
//! use procedural_core::space::Space;
//!
//! pub const FILES: SlotLayout = SlotLayout::new(16, 32);
//!
//! pub fn files_layout() -> procedural_core::bits::BitLayout<u64> {
//!     FILES.layout("files")
//! }
//!
//! pub fn file_id_for(owner: u32, idx: u32) -> u64 {
//!     FILES.compose(owner, idx)
//! }
//!
//! pub fn build_files_space() -> Space<u64> {
//!     FILES.build_space("files")
//! }
//!
//! // Domain-specific: count distribution and derive functions stay here.
//! pub fn file_count_of(owner: u32) -> u32 {
//!     // pareto / lognormal / whatever
//! }
//! ```

use crate::bits::BitLayout;
use crate::space::Space;

/// Layout descriptor for a slot-based service.
///
/// `idx_width + owner_width` must be ≤ 63 to leave room for the
/// `procedural_overlay::Session` sentinel bit at position 63 of the
/// underlying `u64` word.
#[derive(Debug, Clone, Copy)]
pub struct SlotLayout {
    pub idx_width: u8,
    pub owner_width: u8,
}

impl SlotLayout {
    pub const fn new(idx_width: u8, owner_width: u8) -> Self {
        assert!(
            (idx_width as u32 + owner_width as u32) <= 63,
            "SlotLayout: total width must be <= 63 to leave room for the Session sentinel bit"
        );
        Self { idx_width, owner_width }
    }

    /// Maximum number of populated slots per owner: `2^idx_width`.
    pub const fn max_per_owner(&self) -> u32 {
        1u32 << self.idx_width
    }

    pub const fn total_width(&self) -> u32 {
        (self.idx_width + self.owner_width) as u32
    }

    /// Build the `BitLayout<u64>` for this slot service. The field
    /// order is LSB-first: `[idx | owner_mail_id]`. Field names are
    /// fixed (`"idx"`, `"owner_mail_id"`) so consumers can rely on
    /// them when registering Space attributes.
    pub fn layout(&self, _service_name: &str) -> BitLayout<u64> {
        BitLayout::<u64>::new(vec![
            ("idx", self.idx_width),
            ("owner_mail_id", self.owner_width),
        ])
        .expect("slot layout sums to <= 63, well within u64::BITS")
    }

    /// Compose a slot id from `(owner_mail_id, idx)`. Identity:
    /// `(owner_of(compose(o, i)), idx_of(compose(o, i))) == (o, i)`.
    pub fn compose(&self, owner_mail_id: u32, idx: u32) -> u64 {
        self.layout("").compose(&[
            ("owner_mail_id", owner_mail_id as u64),
            ("idx", idx as u64),
        ])
    }

    pub fn owner_of(&self, slot_id: u64) -> u32 {
        self.layout("").extract(slot_id, "owner_mail_id") as u32
    }

    pub fn idx_of(&self, slot_id: u64) -> u32 {
        self.layout("").extract(slot_id, "idx") as u32
    }

    /// Build a `Space<u64>` for this slot service with `idx` and
    /// `owner_mail_id` registered as indexable attributes (so
    /// `Space::find().where_eq("owner_mail_id", X)` works for callers
    /// that *want* the search engine — direct compose is still
    /// recommended for full enumeration).
    pub fn build_space(&self, name: &'static str) -> Space<u64> {
        let layout = self.layout(name);
        let mut space = Space::<u64>::new(name, layout);
        space
            .indexable_attribute::<u32, _>("owner_mail_id", "owner_mail_id", |v| v as u32)
            .expect("register owner_mail_id");
        space
            .indexable_attribute::<u32, _>("idx", "idx", |v| v as u32)
            .expect("register idx");
        space
    }

    /// Enumerate all populated slot ids for `owner`, given a
    /// `count_of(owner)` callback. O(count_of(owner)).
    pub fn ids_for_owner<F: Fn(u32) -> u32>(&self, owner: u32, count_of: F) -> Vec<u64> {
        let n = count_of(owner);
        (0..n).map(|i| self.compose(owner, i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST: SlotLayout = SlotLayout::new(16, 32);

    #[test]
    fn compose_round_trips() {
        let id = TEST.compose(0xDEAD_BEEF, 42);
        assert_eq!(TEST.owner_of(id), 0xDEAD_BEEF);
        assert_eq!(TEST.idx_of(id), 42);
    }

    #[test]
    fn max_per_owner_matches_idx_width() {
        assert_eq!(TEST.max_per_owner(), 1 << 16);
    }

    #[test]
    fn total_width_below_session_sentinel() {
        assert!(TEST.total_width() <= 63);
        assert_eq!(TEST.total_width(), 48);
    }

    #[test]
    fn build_space_registers_indexable_fields() {
        let s = TEST.build_space("test_slot_service");
        let names: Vec<_> = s.attribute_names();
        assert!(names.contains(&"owner_mail_id".to_string()));
        assert!(names.contains(&"idx".to_string()));
    }

    #[test]
    fn ids_for_owner_returns_count_ids_in_order() {
        let owner: u32 = 0xCAFE;
        let ids = TEST.ids_for_owner(owner, |_| 5);
        assert_eq!(ids.len(), 5);
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(TEST.owner_of(id), owner);
            assert_eq!(TEST.idx_of(id), i as u32);
        }
    }
}
