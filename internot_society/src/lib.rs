//! `internot_society`: population and kinship as pure functions of
//! `(seed, id, t)`.
//!
//! This is the R1 prototype of spec `2026-09-29-society-as-a-function.md`
//! §5–§6 (plan: `docs/superpowers/plans/2026-09-30-r1-kinship-prototype.md`).
//! It is built alongside the old `people` service (decision D6).
//!
//! - [`params`]: era-dependent demographic schedules.
//! - [`plan`]: fertility and dissolution plans, apportioned exactly.
//! - [`ledger`]: the integer counts both sides of every relation read.
//! - [`world`]: people, unions, parents and children as keyed lookups.

pub mod ledger;
pub mod params;
pub mod plan;
pub mod world;

pub use ledger::CellKind;
pub use params::{Params, Sex};
pub use world::{PersonId, Union, World};
