//! Canonical "day 0" anchor for the simulation.
//!
//! Every consumer crate that derives absolute timestamps from a stored
//! `day_offset` (mail's `sent_at_of`, calendar's `ta_starts_at`,
//! social's `strength_at`) shares this anchor so dates compose
//! coherently across crates.
//!
//! # Why this is global, not const-injected per-call
//!
//! The CLAUDE.md invariant `now is injectable, never a constant` lives
//! at the World layer: callers that hold a `&World<W>` should call
//! `world.epoch()` to get the (potentially seed-derived) anchor for
//! that World instance. Pure derivers without a World fall back to
//! `default_epoch()`.
//!
//! Future work: when `World::with_epoch(t)` is wired through pure
//! derivers (via threading `epoch` as an arg, the way `t` already
//! flows), this default becomes a true fallback rather than the de
//! facto value.

use chrono::{DateTime, TimeZone, Utc};

/// The default "day 0" anchor: midnight UTC on 2025-01-01.
///
/// Used by all consumer crates as the offset zero for `day_offset`
/// fields. Override at the World level via `World::with_epoch(t)`.
#[inline]
pub fn default_epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()
}
