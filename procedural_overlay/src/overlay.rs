//! Internal storage for attribute overrides + session-allocated entities.
//!
//! Flat-tuple keyed `HashMap` for simplicity: sessions are short-lived
//! (≤ 10⁵ entries) and a per-key hash dominates a nested-map traversal.
//!
//! See spec `procedural-overlay.md` §"Storage".

#![allow(dead_code)] // Wired by session.rs in Step 6.

use procedural_core::word::BitWord;
use std::any::Any;
use std::collections::{HashMap, HashSet};

/// Function pointer that clones a `Box<dyn Any + Send + Sync>` of a known
/// concrete type. Captured at write time when the type is statically known;
/// invoked on read to return owned values without consuming the override.
pub(crate) type CloneFn = fn(&dyn Any) -> Box<dyn Any + Send + Sync>;

pub(crate) struct OverrideEntry {
    pub(crate) value: Box<dyn Any + Send + Sync>,
    pub(crate) clone_fn: CloneFn,
    pub(crate) type_name: &'static str,
}

impl OverrideEntry {
    /// Clone the entry's value into a fresh `Box`. The underlying type is
    /// preserved by `clone_fn`.
    pub(crate) fn clone_value(&self) -> Box<dyn Any + Send + Sync> {
        (self.clone_fn)(&*self.value)
    }
}

#[derive(Default)]
pub(crate) struct Overlay<W: BitWord> {
    /// `(space, id, attr) → entry`. Space/attr are `&'static str`
    /// throughout the codebase (every call site uses literals like
    /// `"messages"` / `"body"`), so storing the static reference
    /// directly avoids any allocation on insert OR lookup.
    overrides: HashMap<(&'static str, W, &'static str), OverrideEntry>,

    /// `(space, id)` of every entity this session allocated. Used by
    /// trajectory serialization and to answer "did this session create x?".
    allocated: HashSet<(&'static str, W)>,
}

impl<W: BitWord> Overlay<W> {
    pub(crate) fn new() -> Self {
        Overlay {
            overrides: HashMap::new(),
            allocated: HashSet::new(),
        }
    }

    pub(crate) fn insert(
        &mut self,
        space: &'static str,
        id: W,
        attr: &'static str,
        entry: OverrideEntry,
    ) {
        self.overrides.insert((space, id, attr), entry);
    }

    pub(crate) fn get(
        &self,
        space: &'static str,
        id: W,
        attr: &'static str,
    ) -> Option<&OverrideEntry> {
        self.overrides.get(&(space, id, attr))
    }

    pub(crate) fn record_allocation(&mut self, space: &'static str, id: W) {
        self.allocated.insert((space, id));
    }

    pub(crate) fn was_allocated(&self, space: &'static str, id: W) -> bool {
        self.allocated.contains(&(space, id))
    }

    pub(crate) fn override_count(&self) -> usize {
        self.overrides.len()
    }

    pub(crate) fn allocated_count(&self) -> usize {
        self.allocated.len()
    }

    /// Iterate `(space, id, attr, &entry)` tuples in unspecified order.
    /// Trajectory serialization uses this.
    pub(crate) fn iter_overrides(
        &self,
    ) -> impl Iterator<Item = (&'static str, W, &'static str, &OverrideEntry)> {
        self.overrides
            .iter()
            .map(|((s, id, a), e)| (*s, *id, *a, e))
    }

    /// Iterate `(space, id)` allocations in unspecified order.
    pub(crate) fn iter_allocations(
        &self,
    ) -> impl Iterator<Item = (&'static str, W)> + '_ {
        self.allocated.iter().map(|(s, id)| (*s, *id))
    }
}

/// Construct a typed `OverrideEntry` for value `V` at a single call site
/// where `V` is statically known. Captures the clone function so reads
/// can later return owned `V` without consuming the entry.
pub(crate) fn make_entry<V>(value: V) -> OverrideEntry
where
    V: Any + Send + Sync + Clone + 'static,
{
    let clone_fn: CloneFn = |any| Box::new(any.downcast_ref::<V>().unwrap().clone());
    OverrideEntry {
        value: Box::new(value),
        clone_fn,
        type_name: std::any::type_name::<V>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_get_returns_entry() {
        let mut o: Overlay<u64> = Overlay::new();
        o.insert("space", 42u64, "attr", make_entry(2.5f64));
        let entry = o.get("space", 42u64, "attr").expect("hit");
        let cloned = entry.clone_value();
        assert_eq!(*cloned.downcast::<f64>().unwrap(), 2.5);
    }

    #[test]
    fn get_returns_none_when_absent() {
        let o: Overlay<u64> = Overlay::new();
        assert!(o.get("space", 0u64, "attr").is_none());
    }

    #[test]
    fn insert_overwrites_existing_entry() {
        let mut o: Overlay<u64> = Overlay::new();
        o.insert("s", 1u64, "a", make_entry(1u64));
        o.insert("s", 1u64, "a", make_entry(2u64));
        let v = *o.get("s", 1u64, "a").unwrap().clone_value().downcast::<u64>().unwrap();
        assert_eq!(v, 2);
    }

    #[test]
    fn distinct_keys_dont_collide() {
        let mut o: Overlay<u64> = Overlay::new();
        o.insert("s", 1u64, "a", make_entry(1u64));
        o.insert("s", 2u64, "a", make_entry(2u64));
        o.insert("s", 1u64, "b", make_entry(3u64));
        o.insert("t", 1u64, "a", make_entry(4u64));
        for (sp, id, attr, expected) in [
            ("s", 1u64, "a", 1u64),
            ("s", 2, "a", 2),
            ("s", 1, "b", 3),
            ("t", 1, "a", 4),
        ] {
            let v = *o
                .get(sp, id, attr)
                .unwrap()
                .clone_value()
                .downcast::<u64>()
                .unwrap();
            assert_eq!(v, expected);
        }
    }

    #[test]
    fn entry_preserves_type_name() {
        let entry = make_entry::<String>("hi".into());
        assert!(entry.type_name.contains("String"));
    }

    #[test]
    fn clone_value_returns_owned_independent_copy() {
        let mut o: Overlay<u64> = Overlay::new();
        o.insert("s", 0u64, "v", make_entry(vec![1u32, 2, 3]));
        let v1 = *o.get("s", 0u64, "v").unwrap().clone_value().downcast::<Vec<u32>>().unwrap();
        let v2 = *o.get("s", 0u64, "v").unwrap().clone_value().downcast::<Vec<u32>>().unwrap();
        assert_eq!(v1, vec![1, 2, 3]);
        assert_eq!(v2, vec![1, 2, 3]);
        // Independent buffers (different addresses).
        assert_ne!(v1.as_ptr(), v2.as_ptr());
    }

    #[test]
    fn record_and_query_allocations() {
        let mut o: Overlay<u64> = Overlay::new();
        assert!(!o.was_allocated("s", 0u64));
        o.record_allocation("s", 0u64);
        o.record_allocation("s", 1u64);
        o.record_allocation("t", 0u64);
        assert!(o.was_allocated("s", 0u64));
        assert!(o.was_allocated("s", 1u64));
        assert!(o.was_allocated("t", 0u64));
        assert!(!o.was_allocated("u", 0u64));
        assert_eq!(o.allocated_count(), 3);
    }

    #[test]
    fn override_count_reflects_inserts() {
        let mut o: Overlay<u64> = Overlay::new();
        assert_eq!(o.override_count(), 0);
        o.insert("s", 0u64, "a", make_entry(1u64));
        o.insert("s", 0u64, "b", make_entry(2u64));
        assert_eq!(o.override_count(), 2);
        // Overwrite same key — count stays.
        o.insert("s", 0u64, "a", make_entry(3u64));
        assert_eq!(o.override_count(), 2);
    }

    #[test]
    fn iter_overrides_yields_every_entry() {
        let mut o: Overlay<u64> = Overlay::new();
        o.insert("s", 1u64, "a", make_entry(10u64));
        o.insert("s", 2u64, "b", make_entry(20u64));
        let mut seen: Vec<(String, u64, String, u64)> = o
            .iter_overrides()
            .map(|(s, id, a, e)| {
                let v = *e.clone_value().downcast::<u64>().unwrap();
                (s.to_string(), id, a.to_string(), v)
            })
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            vec![
                ("s".to_string(), 1, "a".to_string(), 10),
                ("s".to_string(), 2, "b".to_string(), 20),
            ]
        );
    }

    #[test]
    fn iter_allocations_yields_every_pair() {
        let mut o: Overlay<u64> = Overlay::new();
        o.record_allocation("s", 1u64);
        o.record_allocation("t", 5u64);
        let mut seen: Vec<(String, u64)> =
            o.iter_allocations().map(|(s, id)| (s.to_string(), id)).collect();
        seen.sort();
        assert_eq!(seen, vec![("s".to_string(), 1), ("t".to_string(), 5)]);
    }

    #[test]
    fn works_with_u128_word() {
        let mut o: Overlay<u128> = Overlay::new();
        let id: u128 = 0x1234_5678_9ABC_DEF0;
        o.insert("s", id, "v", make_entry("hello".to_string()));
        let s = *o.get("s", id, "v").unwrap().clone_value().downcast::<String>().unwrap();
        assert_eq!(s, "hello");
    }
}
