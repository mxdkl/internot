//! Search primitives: bit-pattern representation and logic minimization.
//!
//! This module holds the building blocks for `find()`-style queries:
//! * [`BitPattern`] describes a set of IDs sharing some pinned bits.
//! * [`range_to_prefixes`] decomposes an integer range into a minimal pattern cover.
//! * [`minimize_patterns`] reduces a value set via Quine-McCluskey logic minimization.
//! * [`PatternEnumerator`] walks a pattern to yield the concrete IDs it covers.
//!
//! Later plans (5c–5f) layer on the `find()` query builder and the
//! `candidates()` similarity API.

pub mod candidates;
pub mod cohort;
pub mod enumerate;
pub mod find_iter;
pub mod minimize;
pub mod op;
pub mod pattern;
pub mod query;
pub mod range;
pub mod results;
pub mod stability;

pub use candidates::{CandidateQuery, CandidateResults};
pub use cohort::Cohort;
pub use enumerate::PatternEnumerator;
pub use find_iter::FindIter;
pub use minimize::minimize_patterns;
pub use op::Op;
pub use pattern::BitPattern;
pub use query::{Bounded, Query, Unbounded};
pub use range::range_to_prefixes;
pub(crate) use results::{Filter, Quantifier};
pub use results::{Results, StabilityMode, Termination, TimeAnchor};
