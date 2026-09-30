//! procedural_core: deterministic, stateless primitives for procedural entity generation.
//!
//! Layer 0 of the framework — domain-agnostic utilities for hashing, bit layouts,
//! trajectory functions, distribution samplers, and pairwise edge functions.

pub mod bits;
pub(crate) mod coerce;
pub mod count;
pub mod dmath;
pub mod edge;
pub mod graph;
pub mod hash;
pub mod key;
pub mod pairing;
pub mod partition;
pub mod perm;
pub mod sample;
pub mod sampler;
pub mod search;
pub mod slot;
pub mod space;
pub mod stream;
pub mod time;
pub mod trajectory;
pub mod word;
pub mod world;

pub use slot::SlotLayout;
pub use time::default_epoch;
pub use word::{BitWord, U256};
