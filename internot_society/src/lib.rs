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
//! - [`household`]: who lives with whom at a time (L3), a view over the
//!   world.
//! - [`residence`]: where households live and who lives in a place (L4,
//!   prototype).

pub mod household;
pub mod ledger;
pub mod names;
pub mod params;
pub mod plan;
pub mod residence;
pub mod world;

pub use household::{Household, Members};
pub use ledger::CellKind;
pub use names::{Middle, Surname};
pub use params::{Heritage, Params, Sex};
pub use world::{PersonId, Union, World};
