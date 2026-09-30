//! `Session<'w, W>` — read/write entry point.
//!
//! A `Session` borrows a [`World`](procedural_core::world::World) and owns
//! an in-memory overlay. Reads check the overlay first and fall through to
//! the underlying procedural functions on miss. Writes append to the
//! overlay, never mutating the procedural floor.
//!
//! See spec `procedural-overlay.md` §"Read path semantics" / "Write path
//! semantics".

use chrono::{DateTime, Utc};
use procedural_core::word::BitWord;
use procedural_core::world::World;
use std::any::{Any, TypeId};

use crate::error::OverlayError;
use crate::ids::Allocator;
use crate::overlay::{make_entry, Overlay, OverrideEntry};

/// Session-scoped read/write view over a `World`.
///
/// Construct with [`Session::new`] (entropy-derived session id) or
/// [`Session::with_session_id`] (explicit, for replay / determinism).
///
/// `Session` borrows the world immutably (`&'w World<W>`); many concurrent
/// sessions on the same world are allowed and isolated from each other.
pub struct Session<'w, W: BitWord> {
    world: &'w World<W>,
    overlay: Overlay<W>,
    allocator: Allocator<W>,
    session_id: u16,
}

impl<'w, W: BitWord> std::fmt::Debug for Session<'w, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("session_id", &self.session_id)
            .field("override_count", &self.overlay.override_count())
            .field("allocated_count", &self.overlay.allocated_count())
            .finish()
    }
}

impl<'w, W: BitWord> Session<'w, W> {
    /// Open a session on `world` with an entropy-derived 15-bit session id.
    /// Use [`Session::with_session_id`] when you need reproducibility.
    pub fn new(world: &'w World<W>) -> Self {
        Self::with_session_id(world, derive_session_id())
    }

    /// Open a session with an explicit 15-bit session id (top bit ignored).
    pub fn with_session_id(world: &'w World<W>, session_id: u16) -> Self {
        let session_id = session_id & ((1u16 << crate::ids::SESSION_ID_BITS) - 1);
        Session {
            world,
            overlay: Overlay::new(),
            allocator: Allocator::new(session_id),
            session_id,
        }
    }

    /// 15-bit session id assigned at construction.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }

    /// Number of attribute overrides currently held.
    pub fn override_count(&self) -> usize {
        self.overlay.override_count()
    }

    /// Number of session-allocated entities.
    pub fn allocated_count(&self) -> usize {
        self.overlay.allocated_count()
    }

    /// True iff `id` was allocated by some session (not necessarily this
    /// one). Pure check on the sentinel bit; doesn't consult the overlay.
    pub fn is_session_allocated(id: W) -> bool {
        crate::ids::is_session_allocated(id)
    }

    /// Read an attribute. Checks the overlay first; on miss falls through
    /// to `World::attribute_value`.
    pub fn attribute_value<V: Any + 'static>(
        &self,
        space: &'static str,
        id: W,
        attr: &'static str,
        t: Option<DateTime<Utc>>,
    ) -> Result<V, OverlayError> {
        let space_ref = self
            .world
            .space(space)
            .map_err(|_| OverlayError::UnknownSpace(space.to_string()))?;
        let expected_type = space_ref
            .attribute_type_id(attr)
            .ok_or_else(|| OverlayError::UnknownAttribute {
                space: space.to_string(),
                attr: attr.to_string(),
            })?;
        if expected_type != TypeId::of::<V>() {
            return Err(OverlayError::TypeMismatch {
                space: space.to_string(),
                attr: attr.to_string(),
                expected: space_ref.attribute_type_name(attr).unwrap_or("?"),
                actual: std::any::type_name::<V>(),
            });
        }

        if let Some(entry) = self.overlay.get(space, id, attr) {
            let cloned: Box<dyn Any + Send + Sync> = entry.clone_value();
            return Ok(*cloned
                .downcast::<V>()
                .expect("clone_fn captured at write time preserves the concrete type"));
        }

        self.world
            .attribute_value::<V>(space, id, attr, t)
            .map_err(OverlayError::World)
    }

    /// Write an attribute override. The value type must match the
    /// attribute's registered type (validated via
    /// [`Space::attribute_type_id`](procedural_core::space::Space::attribute_type_id)).
    pub fn write_attribute<V: Any + Send + Sync + Clone + 'static>(
        &mut self,
        space: &'static str,
        id: W,
        attr: &'static str,
        value: V,
    ) -> Result<(), OverlayError> {
        let space_ref = self
            .world
            .space(space)
            .map_err(|_| OverlayError::UnknownSpace(space.to_string()))?;
        let expected_type = space_ref
            .attribute_type_id(attr)
            .ok_or_else(|| OverlayError::UnknownAttribute {
                space: space.to_string(),
                attr: attr.to_string(),
            })?;
        if expected_type != TypeId::of::<V>() {
            return Err(OverlayError::TypeMismatch {
                space: space.to_string(),
                attr: attr.to_string(),
                expected: space_ref.attribute_type_name(attr).unwrap_or("?"),
                actual: std::any::type_name::<V>(),
            });
        }
        self.overlay.insert(space, id, attr, make_entry(value));
        Ok(())
    }

    /// Allocate a fresh id in this session's range. Returned ids carry the
    /// sentinel bit set (top bit of `W`) and never collide with
    /// procedurally-derivable ids — provided application bit layouts respect
    /// the documented `total_width() ≤ W::BITS - 1` constraint.
    ///
    /// `space` is recorded for trajectory purposes; subsequent
    /// [`Session::write_attribute`] calls populate the entity's attributes.
    pub fn allocate_entity(&mut self, space: &'static str) -> W {
        let id = self.allocator.allocate();
        self.overlay.record_allocation(space, id);
        id
    }

    /// True iff this session allocated `(space, id)`.
    pub fn was_allocated(&self, space: &'static str, id: W) -> bool {
        self.overlay.was_allocated(space, id)
    }

    /// Internal: insert a pre-built `OverrideEntry`. Used by
    /// [`crate::trajectory::replay`] to reconstruct overrides that were
    /// captured by-value in a `Trajectory`.
    ///
    /// Replay-only path: takes `&str` and leaks it to `&'static str`
    /// since trajectory deserialization produces owned `String`s but
    /// the overlay key requires `'static`. The leak is bounded by the
    /// trajectory size and only happens during replay (cold path).
    pub(crate) fn insert_raw_override(
        &mut self,
        space: &str,
        id: W,
        attr: &str,
        entry: OverrideEntry,
    ) {
        let space: &'static str = Box::leak(space.to_string().into_boxed_str());
        let attr: &'static str = Box::leak(attr.to_string().into_boxed_str());
        self.overlay.insert(space, id, attr, entry);
    }

    /// Internal: re-record an allocation and bump the allocator counter
    /// past `id` so subsequent `allocate_entity` calls don't collide.
    /// Replay-only; same `Box::leak` strategy as `insert_raw_override`.
    pub(crate) fn replay_allocation(&mut self, space: &str, id: W) {
        let space: &'static str = Box::leak(space.to_string().into_boxed_str());
        self.overlay.record_allocation(space, id);
        self.allocator.observe(id);
    }

    /// Borrow the underlying overlay for trajectory serialization.
    pub(crate) fn overlay(&self) -> &Overlay<W> {
        &self.overlay
    }
}

/// Derive a 15-bit session id from process metadata at call time.
/// Not cryptographically random; just enough entropy that two sessions
/// in the same process get different ids in practice.
fn derive_session_id() -> u16 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let mut h = DefaultHasher::new();
    (n, nanos, pid).hash(&mut h);
    (h.finish() as u16) & ((1u16 << crate::ids::SESSION_ID_BITS) - 1)
}

#[cfg(test)]
mod tests {
    // Most session behavior is exercised by the integration tests under
    // `tests/session_io.rs` (a real `World` is needed to test reads).
    // Unit tests here cover only the bits that don't need a `World`.

    use super::*;

    #[test]
    fn with_session_id_truncates_to_15_bits() {
        let world: World<u64> = World::new();
        let s = Session::with_session_id(&world, 0xFFFF);
        assert_eq!(s.session_id(), 0x7FFF);
    }

    #[test]
    fn new_assigns_some_session_id() {
        let world: World<u64> = World::new();
        let s = Session::new(&world);
        // Just check it fits in 15 bits; entropy means we don't pin a value.
        assert!(s.session_id() < (1 << 15));
    }

    #[test]
    fn override_count_starts_at_zero() {
        let world: World<u64> = World::new();
        let s = Session::new(&world);
        assert_eq!(s.override_count(), 0);
        assert_eq!(s.allocated_count(), 0);
    }

    #[test]
    fn is_session_allocated_static_check() {
        assert!(!Session::<u64>::is_session_allocated(0u64));
        assert!(Session::<u64>::is_session_allocated(1u64 << 63));
    }
}
