//! `internot_society`: population and kinship as pure functions of
//! `(seed, id, t)`, with nothing stored per person.
//!
//! - [`params`]: the world pack's typed sections (schedules, mortality,
//!   unions, fertility, migration, places, names).
//! - [`names`]: the pack's name data.
//! - [`mono`]: the monotone world. Every coupling between index spaces
//!   preserves order, deaths are quantile slots of classes, and births
//!   choose mothers among the living, so counts a search asks for are
//!   closed form at any population (`thinking/claude/004`–`011`).

pub mod mono;
pub mod names;
pub mod params;

pub use params::{Heritage, Params, Sex};
