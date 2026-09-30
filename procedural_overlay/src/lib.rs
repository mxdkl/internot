//! `procedural_overlay` — session-scoped mutation on top of `procedural_core`.
//!
//! `procedural_core` is read-only by design: every value is a pure function
//! of `(id, key, t)`. This crate adds a [`Session`] that wraps a borrowed
//! [`World`](procedural_core::world::World), intercepts reads with an
//! in-memory overlay, and absorbs writes into that overlay. The procedural
//! floor stays pure; mutations live entirely in the session.
//!
//! ## Quick start
//!
//! ```no_run
//! use procedural_core::bits::BitLayout;
//! use procedural_core::space::Space;
//! use procedural_core::world::World;
//! use procedural_overlay::Session;
//!
//! // 1. Build a world with at least one space + attribute.
//! let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
//! let mut people = Space::<u64>::new("people", layout);
//! people.attribute::<f64, _>("score", |id: u64| id as f64).unwrap();
//! let mut world: World<u64> = World::new();
//! world.register(people).unwrap();
//!
//! // 2. Open a session on it.
//! let mut session = Session::new(&world);
//!
//! // 3. Reads fall through to the procedural floor.
//! let v: f64 = session.attribute_value("people", 42u64, "score", None).unwrap();
//! assert_eq!(v, 42.0);
//!
//! // 4. Writes go into the overlay; subsequent reads see them.
//! session.write_attribute::<f64>("people", 42u64, "score", 999.0).unwrap();
//! let v: f64 = session.attribute_value("people", 42u64, "score", None).unwrap();
//! assert_eq!(v, 999.0);
//!
//! // 5. Allocating a fresh entity returns an id with the sentinel bit set.
//! let new_id = session.allocate_entity("people");
//! assert!(Session::<u64>::is_session_allocated(new_id));
//! ```
//!
//! ## Session-id sentinel convention
//!
//! Session-allocated ids must never collide with procedurally-derivable
//! ids. The convention is a **high-bit sentinel**:
//!
//! | `W` | Layout |
//! |---|---|
//! | `u64`  | `[1 : 1bit][session_id : 15bit][counter : 48bit]` |
//! | `u128` | `[1 : 1bit][session_id : 15bit][counter : 112bit]` |
//!
//! Procedural ids must keep the top bit clear. **Application bit layouts
//! must therefore reserve the top bit** (`BitLayout::total_width() ≤
//! W::BITS - 1`). Concretely:
//!
//! - For a `u64` world, your bit fields' widths must sum to ≤ 63.
//! - For a `u128` world, ≤ 127.
//!
//! All current consumer spaces (`internot_mail::messages`,
//! `internot_calendar::events`, `internot_calendar::rsvp_flags`) comply
//! by construction. New spaces consumed by a `Session` should follow
//! the same constraint.
//!
//! [`Session::is_session_allocated`] is a static check on the sentinel
//! bit; use it to defensively detect "did this id come from someone's
//! allocator?" without needing the originating session.
//!
//! ## What's in scope (v1)
//!
//! - Per-attribute writes that override the procedural value for one
//!   `(space, id, attr)` triple.
//! - Session-allocated ids with collision-free counters.
//! - In-memory record/replay via [`Trajectory`] / [`replay`].
//! - Type-checked writes (validated against the registered attribute type).
//!
//! ## What's not in scope (yet)
//!
//! - Persistent on-disk trajectories (no serde for `Box<dyn Any>` in v1).
//! - Multi-session merge / CRDT-style overlay composition.
//! - Overlay-aware cross-space attribute evaluation: when a
//!   `cross_space_attribute` closure calls `world.attribute_value`, that
//!   read goes to the procedural floor with no overlay awareness.
//!   Acceptable for append-only services (mail, calendar); revisit when a
//!   service needs mutation visibility through cross-space links.
//! - Tombstones / deletions on procedural ids (only override-with-new-value).
//! - Overlay over `Space::related` and `Space::find` queries (only direct
//!   attribute reads in v1).
//!
//! ## Spec
//!
//! Full design under `docs/superpowers/specs/2026-04-25-procedural-overlay.md`.

pub(crate) mod ids;
pub(crate) mod overlay;

pub mod error;
pub mod session;
pub mod trajectory;

pub use error::OverlayError;
pub use session::Session;
pub use trajectory::{replay, Trajectory, TrajectoryEvent};
