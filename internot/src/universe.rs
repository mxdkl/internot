//! `Universe` — process-wide runtime that all views borrow.
//!
//! Holds the society world (built once per process and shared), the
//! mutex-guarded `SessionState` (the mutation trace) and the current
//! simulated time.
//!
//! Per CLAUDE.md invariant 3 ("`now` is injectable, never a constant"),
//! `now` is a field on Universe — not a global. Tests pin it; the
//! production server pins it too (currently 2025-06-30T09:00:00Z).
//!
//! The Universe is constructed once per process. All transports
//! (`internot_mcp`, etc.) build it at startup and pass `&Universe`
//! into every view.

use chrono::{DateTime, TimeZone, Utc};
use parking_lot::Mutex;

/// The simulation's "current time" anchor. Injected, never a constant.
/// (Per invariant 3 in CLAUDE.md.)
pub const DEFAULT_NOW: fn() -> DateTime<Utc> = || {
    Utc.with_ymd_and_hms(2025, 6, 30, 9, 0, 0)
        .single()
        .expect("default now is unambiguous in UTC")
};

/// Container for every per-process mutable session. After the
/// 2026-05-14 nuke, no mutating services remain — `SessionState`
/// carries only the trace until services come back in the
/// social-graph rebuild. New mutating services add their session
/// fields here.
pub struct SessionState {
    pub trace: MutationTrace,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState {
            trace: MutationTrace::default(),
        }
    }
}

/// Cumulative trace of mutations performed in this process. Scenario
/// verdict logic reads this; the agent doesn't need to. Empty until
/// the rebuilt services repopulate it with their typed event lists.
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct MutationTrace {}

/// The single shared runtime every view borrows. Transports build it
/// once at startup.
///
/// It holds the procedural floor (the society world), the mutable
/// `sessions` and the simulated `now`.
pub struct Universe {
    pub sessions: Mutex<SessionState>,
    pub now: DateTime<Utc>,
    /// The society world (people and kinship, the monotone world),
    /// shared by every Universe of the process with the same pack and seed.
    pub society: &'static crate::society::Society,
}

impl Universe {
    /// Build a Universe at the canonical time anchor.
    pub fn new() -> Self {
        Universe::with_now(DEFAULT_NOW())
    }

    /// A Universe at `now` on the society world of `INTERNOT_PACK`,
    /// `INTERNOT_SEED` and `INTERNOT_SCALE` (default `us`, 42, 1).
    pub fn with_now(now: DateTime<Utc>) -> Self {
        Self::with_society(crate::society::Society::from_env(), now)
    }

    /// A Universe at `now` on embedded pack `pack` (seed 42): tests use
    /// `us-tiny`.
    pub fn for_pack(pack: &str, now: DateTime<Utc>) -> Self {
        Self::with_society(
            crate::society::Society::shared(pack, crate::society::DEFAULT_SEED, crate::society::DEFAULT_SCALE),
            now,
        )
    }

    fn with_society(society: &'static crate::society::Society, now: DateTime<Utc>) -> Self {
        Universe {
            sessions: Mutex::new(SessionState::new()),
            now,
            society,
        }
    }

    /// Override `now` after construction (for tests that want to
    /// step time without rebuilding the World).
    pub fn set_now(&mut self, now: DateTime<Utc>) {
        self.now = now;
    }
}

impl Default for Universe {
    fn default() -> Self {
        Self::new()
    }
}
