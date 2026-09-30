//! `Trajectory<W>` and `replay()` — record/replay session events.
//!
//! v1 is in-memory only: a `Trajectory` is moved out of one session and
//! into [`replay`], which reconstructs an equivalent session against the
//! same `World`. Persistent serialization is a v2 concern (would require
//! a TypeId → (serialize, deserialize) registry).
//!
//! See spec `procedural-overlay.md` §"Trajectory & replay".

use procedural_core::word::BitWord;
use procedural_core::world::World;

use crate::error::OverlayError;
use crate::overlay::{CloneFn, OverrideEntry};
use crate::session::Session;

/// Single recordable event.
pub enum TrajectoryEvent<W: BitWord> {
    Allocated {
        space: String,
        id: W,
    },
    Write {
        space: String,
        id: W,
        attr: String,
        value: Box<dyn std::any::Any + Send + Sync>,
        clone_fn: CloneFn,
        type_name: &'static str,
    },
}

impl<W: BitWord> std::fmt::Debug for TrajectoryEvent<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrajectoryEvent::Allocated { space, id } => f
                .debug_struct("Allocated")
                .field("space", space)
                .field("id", id)
                .finish(),
            TrajectoryEvent::Write {
                space,
                id,
                attr,
                type_name,
                ..
            } => f
                .debug_struct("Write")
                .field("space", space)
                .field("id", id)
                .field("attr", attr)
                .field("type_name", type_name)
                .field("value", &"<dyn Any>")
                .finish(),
        }
    }
}

/// Record of a session's actions, suitable for [`replay`].
///
/// `events` are in insertion order: allocations and writes interleave
/// in the order they were performed. Replay reissues them in the same
/// order against a fresh session.
pub struct Trajectory<W: BitWord> {
    pub session_id: u16,
    pub events: Vec<TrajectoryEvent<W>>,
}

impl<W: BitWord> std::fmt::Debug for Trajectory<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trajectory")
            .field("session_id", &self.session_id)
            .field("event_count", &self.events.len())
            .finish()
    }
}

impl<W: BitWord> Session<'_, W> {
    /// Consume the session and produce a `Trajectory` whose `replay`
    /// against the same world reproduces every read this session would
    /// have served.
    ///
    /// The trajectory's event order is unspecified across calls (the
    /// underlying `HashMap`/`HashSet` iteration order is arbitrary), but
    /// replay is order-insensitive — writes overwrite, allocations are
    /// idempotent — so reads after replay match the original session
    /// regardless.
    pub fn into_trajectory(self) -> Trajectory<W> {
        let session_id = self.session_id();
        let overlay = self.overlay();
        let mut events: Vec<TrajectoryEvent<W>> =
            Vec::with_capacity(overlay.allocated_count() + overlay.override_count());
        for (space, id) in overlay.iter_allocations() {
            events.push(TrajectoryEvent::Allocated {
                space: space.to_string(),
                id,
            });
        }
        for (space, id, attr, entry) in overlay.iter_overrides() {
            events.push(TrajectoryEvent::Write {
                space: space.to_string(),
                id,
                attr: attr.to_string(),
                value: entry.clone_value(),
                clone_fn: entry.clone_fn,
                type_name: entry.type_name,
            });
        }
        Trajectory { session_id, events }
    }
}

/// Reconstruct a session by replaying every event in `trajectory` against
/// `world`. The new session has the same `session_id` and produces the
/// same reads as the original.
pub fn replay<'w, W: BitWord>(
    world: &'w World<W>,
    trajectory: Trajectory<W>,
) -> Result<Session<'w, W>, OverlayError> {
    let mut session = Session::with_session_id(world, trajectory.session_id);
    // Allocations first so the allocator counter advances past every
    // observed id before any write that might reference one.
    for event in &trajectory.events {
        if let TrajectoryEvent::Allocated { space, id } = event {
            session.replay_allocation(space, *id);
        }
    }
    for event in trajectory.events {
        if let TrajectoryEvent::Write {
            space,
            id,
            attr,
            value,
            clone_fn,
            type_name,
        } = event
        {
            // Validate write target still exists in the world's schema.
            // (Replay against a world that's missing the space/attr is a
            // user error; surface it rather than silently dropping.)
            let space_ref = world
                .space(&space)
                .map_err(|_| OverlayError::UnknownSpace(space.clone()))?;
            let _ = space_ref
                .attribute_type_id(&attr)
                .ok_or_else(|| OverlayError::UnknownAttribute {
                    space: space.clone(),
                    attr: attr.clone(),
                })?;
            session.insert_raw_override(
                &space,
                id,
                &attr,
                OverrideEntry {
                    value,
                    clone_fn,
                    type_name,
                },
            );
        }
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    // Unit tests cover the trajectory shape; round-trip behavior is
    // exercised by integration tests under `tests/trajectory.rs`.
    use super::*;

    #[test]
    fn trajectory_event_debug_works() {
        let e: TrajectoryEvent<u64> = TrajectoryEvent::Allocated {
            space: "s".into(),
            id: 0u64,
        };
        let _ = format!("{:?}", e);
    }

    #[test]
    fn empty_trajectory_replays_into_empty_session() {
        use procedural_core::world::World;
        let world: World<u64> = World::new();
        let t = Trajectory::<u64> {
            session_id: 7,
            events: Vec::new(),
        };
        let s = replay(&world, t).unwrap();
        assert_eq!(s.session_id(), 7);
        assert_eq!(s.override_count(), 0);
        assert_eq!(s.allocated_count(), 0);
    }
}
