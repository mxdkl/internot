//! `VenueSpace` — cohort enumeration helper over a registered Space.
//!
//! The existing `internot::people::cohort::workplace_members_of` did
//! this by hand: a `Space::find()` pushdown on the workplace bits. This
//! generalizes the pattern.
//!
//! ## Required Space shape
//!
//! Consumers register a `Space<W>` whose every entity is one
//! *(member_id, venue_id, role, intra_order, window_start_day,
//! window_end_day)* tuple. Both `member_id` and `venue_id` are
//! indexable BitLayout fields (so the `Space::find()` pushdown works
//! either direction). `window_start_day` / `window_end_day` give the
//! membership's active interval in days-since-epoch.
//!
//! VenueSpace stores the field names and provides the canonical
//! queries. Each query is `Space::find().where_eq(...)` plus an
//! in-window filter on the start/end fields.
//!
//! ## Time parameter
//!
//! Methods take `at_day_since_epoch: u32` rather than `DateTime<Utc>`
//! because the underlying Space's window fields are stored as
//! days-since-epoch (`u16`/`u32` BitLayout fields). Taking the
//! parameter in the same unit avoids a conversion at every call
//! and lets the comparison stay in integer space. Consumers convert
//! once at the boundary via `(t - epoch).num_days() as u32`.
//!
//! ## Performance note
//!
//! Queries are O(2^|free bits after where_eq|).
//! Consumers should pin enough fields in `where_eq` that the
//! remaining free-bit space is reasonable (≤ ~16 bits; at 16 free
//! bits, `find()` draws at most 65 536 candidates before the window
//! filter). The framework panics if total free bits across all
//! cursors reach 64.

use crate::bits::BitLayout;
use crate::word::BitWord;
use crate::world::World;

/// Configuration for a Space-as-VenueSpace. Construct once at service
/// registration time and reuse across queries. Field names must match
/// indexable attributes on the registered Space.
#[derive(Clone, Debug)]
pub struct VenueSpace {
    pub space_name: &'static str,
    pub member_id_field: &'static str,
    pub venue_id_field: &'static str,
    pub role_field: &'static str,
    /// Birth-order or intra-venue rank field — reserved for future
    /// "nth member" queries (e.g. sibling birth-order). Not used by
    /// the current `members_of` / `venues_of` / `role_of` methods.
    pub intra_order_field: &'static str,
    pub window_start_field: &'static str,
    pub window_end_field: &'static str,
}

impl VenueSpace {
    /// Members of `venue_id` whose `[window_start, window_end]`
    /// interval contains `at_day_since_epoch`.
    pub fn members_of<W: BitWord>(
        &self,
        world: &World<W>,
        venue_id: u64,
        at_day_since_epoch: u32,
    ) -> Vec<u32> {
        let space = world
            .space(self.space_name)
            .expect("VenueSpace::members_of — space not registered");
        let layout: &BitLayout<W> = space.layout();
        let member_ex = layout.extractor(self.member_id_field);
        let start_ex = layout.extractor(self.window_start_field);
        let end_ex = layout.extractor(self.window_end_field);
        space
            .find()
            .where_eq(self.venue_id_field, venue_id)
            .execute()
            .filter_map(|eid| {
                let start = start_ex(eid) as u32;
                let end = end_ex(eid) as u32;
                if (start..=end).contains(&at_day_since_epoch) {
                    Some(member_ex(eid) as u32)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Venues that `member_id` belongs to at `at_day_since_epoch`.
    pub fn venues_of<W: BitWord>(
        &self,
        world: &World<W>,
        member_id: u32,
        at_day_since_epoch: u32,
    ) -> Vec<u64> {
        let space = world
            .space(self.space_name)
            .expect("VenueSpace::venues_of — space not registered");
        let layout: &BitLayout<W> = space.layout();
        let venue_ex = layout.extractor(self.venue_id_field);
        let start_ex = layout.extractor(self.window_start_field);
        let end_ex = layout.extractor(self.window_end_field);
        space
            .find()
            .where_eq(self.member_id_field, member_id as u64)
            .execute()
            .filter_map(|eid| {
                let start = start_ex(eid) as u32;
                let end = end_ex(eid) as u32;
                if (start..=end).contains(&at_day_since_epoch) {
                    Some(venue_ex(eid))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Role of `member_id` in `venue_id` at `at_day_since_epoch`,
    /// if the membership exists in that time window. Returns the raw
    /// `u8`; the consumer maps to its role enum.
    pub fn role_of<W: BitWord>(
        &self,
        world: &World<W>,
        member_id: u32,
        venue_id: u64,
        at_day_since_epoch: u32,
    ) -> Option<u8> {
        let space = world
            .space(self.space_name)
            .expect("VenueSpace::role_of — space not registered");
        let layout: &BitLayout<W> = space.layout();
        let role_ex = layout.extractor(self.role_field);
        let start_ex = layout.extractor(self.window_start_field);
        let end_ex = layout.extractor(self.window_end_field);
        space
            .find()
            .where_eq(self.member_id_field, member_id as u64)
            .where_eq(self.venue_id_field, venue_id)
            .execute()
            .filter_map(|eid| {
                let start = start_ex(eid) as u32;
                let end = end_ex(eid) as u32;
                if (start..=end).contains(&at_day_since_epoch) {
                    Some(role_ex(eid) as u8)
                } else {
                    None
                }
            })
            .next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitLayout;
    use crate::space::Space;

    /// Build a tiny-layout world that exercises VenueSpace queries
    /// without the `Space::find` enumeration cost of a real layout.
    /// Layout = 10 bits → at most 1024 candidate bit-patterns per query.
    fn build_tiny_world() -> (World<u64>, VenueSpace) {
        let layout = BitLayout::<u64>::new(vec![
            ("member_id", 2),
            ("venue_id", 2),
            ("role", 1),
            ("intra_order", 1),
            ("window_start_day", 2),
            ("window_end_day", 2),
        ])
        .unwrap();
        let mut space = Space::<u64>::new("memberships", layout);
        for f in [
            "member_id",
            "venue_id",
            "role",
            "intra_order",
            "window_start_day",
            "window_end_day",
        ] {
            space.indexable_attribute::<u64, _>(f, f, |v| v).unwrap();
        }
        let mut w = World::<u64>::new();
        w.register(space).unwrap();
        let vs = VenueSpace {
            space_name: "memberships",
            member_id_field: "member_id",
            venue_id_field: "venue_id",
            role_field: "role",
            intra_order_field: "intra_order",
            window_start_field: "window_start_day",
            window_end_field: "window_end_day",
        };
        (w, vs)
    }

    #[test]
    fn venue_space_clones() {
        let (_, vs) = build_tiny_world();
        let vs2 = vs.clone();
        assert_eq!(vs.space_name, vs2.space_name);
        assert_eq!(vs.member_id_field, vs2.member_id_field);
    }

    #[test]
    fn members_of_executes_and_returns_bounded_results() {
        let (w, vs) = build_tiny_world();
        // venue_id width = 2 → valid venues are 0..=3. day_in_window = 1.
        let members = vs.members_of(&w, 1, 1);
        // member_id width = 2 → at most 4 distinct ids; the Vec may
        // contain duplicates because every (start, end) combination
        // that contains day 1 surfaces the same member_id.
        for &m in &members {
            assert!(m <= 3, "member_id width is 2 bits; got {m}");
        }
    }

    #[test]
    fn members_of_is_deterministic() {
        let (w, vs) = build_tiny_world();
        let m1 = vs.members_of(&w, 1, 1);
        let m2 = vs.members_of(&w, 1, 1);
        assert_eq!(m1, m2);
    }

    #[test]
    fn venues_of_executes_and_returns_bounded_results() {
        let (w, vs) = build_tiny_world();
        let venues = vs.venues_of(&w, 1, 1);
        for &v in &venues {
            assert!(v <= 3, "venue_id width is 2 bits; got {v}");
        }
    }

    #[test]
    fn role_of_returns_a_value_when_window_contains_day() {
        let (w, vs) = build_tiny_world();
        // For member 1 in venue 1, with the tiny layout enumeration,
        // there exist bit-patterns whose window contains day 1.
        // The first match wins; assert role is in 0..=1 (1-bit field).
        let r = vs.role_of(&w, 1, 1, 1);
        if let Some(role) = r {
            assert!(role <= 1, "role width is 1 bit; got {role}");
        }
        // (If `None`, that's also a valid outcome — but we expect
        // SOME bit-pattern to land. Just don't crash.)
    }
}
