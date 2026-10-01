//! World packs: every statistic a world uses, in editable RON files
//! (spec `docs/superpowers/specs/2026-10-01-world-packs.md`).
//!
//! A **pack** is a set of RON section files (`mortality.ron`,
//! `heritage.ron`, ...) and data files (`data/names.bin`), named by a
//! directory. Its `world.ron` may `extend` another pack, in which case the
//! child's sections are merged into the parent's ([`merge`]). Mechanisms
//! live in code; a pack only supplies numbers, lists and names.
//!
//! This crate is the generic part:
//! - sources ([`Source`]): directories and packs embedded in a binary;
//! - loading, `extends` and merging ([`Pack::load`]);
//! - errors that name the pack, file, line or field ([`DefError`]);
//! - a fingerprint of the merged pack ([`Pack::fingerprint`]);
//! - the value vocabulary ([`value`]): year series, steps, ranges, per-sex
//!   pairs.
//!
//! Consumers (`internot_society`, later the services) define their sections
//! as serde structs and read them with [`Pack::section`].

mod error;
mod pack;
pub mod value;

pub use error::DefError;
pub use pack::{merge, Dir, Embedded, EmbeddedFile, Pack, Source};
pub use value::{Bands, BySex, Ranges, Series, Steps, VecSeries};
