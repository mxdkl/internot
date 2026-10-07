//! procedural_core: deterministic, stateless math for procedural worlds.
//!
//! Every function is a pure function of its inputs, with golden values and
//! property tests. Float math goes through [`dmath`] (pinned `libm`), so
//! results are bit-identical across machines.
//!
//! - Keys and hashes: [`key`], [`hash`], [`word`].
//! - Sampling and laws: [`sample`], [`curve`], [`pmf`], [`life`],
//!   [`liability`], [`interp`], [`fit`], [`quantile`].
//! - Exact counts and splits: [`partition`], [`count`], [`lattice`],
//!   [`table`].
//! - Bijections and pairings: [`perm`], [`pairing`].
//! - Event streams and regeneration: [`stream`].

pub mod count;
pub mod curve;
pub mod dmath;
pub mod fit;
pub mod geo;
pub mod hash;
pub mod interp;
pub mod key;
pub mod lattice;
pub mod liability;
pub mod life;
pub mod pairing;
pub mod partition;
pub mod perm;
pub mod pmf;
pub mod quantile;
pub mod sample;
pub mod stream;
pub mod table;
pub mod word;

pub use word::{BitWord, U256};
