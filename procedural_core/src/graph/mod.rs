//! `procedural_core::graph` — domain-free social-graph primitives.
//!
//! Layer 0.5 of the framework, building on hash / trajectory / space.
//! Consumers (e.g. `internot::social`) provide the ontology (what's a
//! venue, what kinds of ties exist) and instantiate these primitives
//! with their own role enums and parameter tables.
//!
//! ## Members
//!
//! - [`util`] — canonical pair-keying (symmetric in `(a, b)`).
//! - [`tie`] — [`Tie`] struct: the edge.
//! - [`venue`] — [`VenueSpace`]: cohort enumeration over a registered Space.
//! - [`strength`] — [`TieStrengthProfile`] + [`tie_strength`] over time.
//! - [`intensity`] — [`CommIntensity`] + [`comm_intensity`].
//! - [`events`] — deterministic, window-independent event enumeration.
//! - [`matching`] — Irving stable-roommates pairing. **Deprecated:** exact
//!   stable matching is provably non-local; use [`crate::pairing::Coupling`].
//!
//! See the spec in
//! `docs/superpowers/specs/2026-05-14-social-graph-substrate.md` and
//! the implementation plan in
//! `docs/superpowers/plans/2026-05-14-phase-0-graph-primitives.md`.
//!
//! ## Design principles
//!
//! 1. Everything pure: `f(id, key, t)`; no RNG, no stored state.
//! 2. Pair-keyed hashing is canonical (symmetric in argument order)
//!    so reciprocity is automatic.
//! 3. Time-varying quantities compose existing `trajectory::*` primitives
//!    so stability radii stack via `stability::min_of`.
//!
pub mod events;
pub mod intensity;
pub mod matching;
pub mod strength;
pub mod tie;
pub mod util;
pub mod venue;

pub use events::{enumerate_events, enumerate_events_keyed, CommEvent};
pub use intensity::{comm_intensity, CommIntensity, PersonalityProjection};
#[allow(deprecated)]
pub use matching::stable_roommates_match;
pub use strength::{tie_strength, TieStrengthProfile};
pub use tie::{Tie, TIE_KIND_UNSPECIFIED};
pub use util::{canonical_pair, pair_hash_float};
pub use venue::VenueSpace;
