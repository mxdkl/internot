//! `Universe` — process-wide runtime that all views borrow.
//!
//! Holds the leaked `&'static World<u128>` (so session-bearing types
//! can hold `'static` borrows without lifetime gymnastics), the
//! mutex-guarded `SessionState` (mail + calendar overlays + mutation
//! trace), and the current simulated time.
//!
//! Per CLAUDE.md invariant 3 ("`now` is injectable, never a constant"),
//! `now` is a field on Universe — not a global. Tests pin it; the
//! production server pins it too (currently 2025-06-30T09:00:00Z).
//!
//! The Universe is constructed once per process. All transports
//! (`internot_mcp`, etc.) build it at startup and pass `&Universe`
//! into every view.

use std::sync::OnceLock;

use chrono::{DateTime, TimeZone, Utc};
use parking_lot::Mutex;
use procedural_core::word::{U256, U512};
use procedural_core::world::World;

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
    pub fn new(_world_u128: &'static World<u128>, _world_u256: &'static World<U256>) -> Self {
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
/// It holds the procedural floor (the society world and the three
/// `world*` containers), the mutable `sessions` and the simulated `now`.
pub struct Universe {
    /// Leaked `&'static` so service Sessions (which borrow `'w` from
    /// the World) can hold `'static` borrows. One Universe per
    /// process; the leak lives for the process. No lifetime concerns.
    pub world: &'static World<u128>,
    /// Parallel world for spaces with bit layouts > 128 bits. Narrow
    /// services keep using `world` (u128) to preserve their bitsearch
    /// performance.
    pub world_u256: &'static World<U256>,
    /// World for spaces with bit layouts that need >256 bits of carry —
    /// currently the People space (post-2026-05 substrate widening).
    /// People's BitLayout is 32 bits but person_id is U512-wide because
    /// the upper bits cache derived attributes.
    pub world_u512: &'static World<U512>,
    pub sessions: Mutex<SessionState>,
    pub now: DateTime<Utc>,
    /// The society world (people, kinship, households, names, residence),
    /// shared by every Universe of the process with the same pack and seed.
    pub society: &'static crate::society::Society,
}

impl Universe {
    /// Build a Universe at the canonical time anchor. Spaces are
    /// registered onto the world by each service's `register` fn (see
    /// `crate::register_all`).
    pub fn new() -> Self {
        Universe::with_now(DEFAULT_NOW())
    }

    /// A Universe at `now` on the society world of `INTERNOT_PACK` and
    /// `INTERNOT_SEED` (default `us`, 42).
    pub fn with_now(now: DateTime<Utc>) -> Self {
        Self::with_society(crate::society::Society::from_env(), now)
    }

    /// A Universe at `now` on embedded pack `pack` (seed 42): tests use
    /// `us-tiny`.
    pub fn for_pack(pack: &str, now: DateTime<Utc>) -> Self {
        Self::with_society(
            crate::society::Society::shared(pack, crate::society::DEFAULT_SEED),
            now,
        )
    }

    fn with_society(society: &'static crate::society::Society, now: DateTime<Utc>) -> Self {
        let world = build_world();
        let world_u256 = build_world_u256();
        let world_u512 = build_world_u512();
        Universe {
            world,
            world_u256,
            world_u512,
            sessions: Mutex::new(SessionState::new(world, world_u256)),
            now,
            society,
        }
    }

    /// Build a Universe sharing an already-constructed world. Used by
    /// transports that hold many concurrent sessions, each with its
    /// own `SessionState` against the same procedural floor.
    pub fn from_world(world: &'static World<u128>, now: DateTime<Utc>) -> Self {
        let world_u256 = build_world_u256();
        let world_u512 = build_world_u512();
        Universe {
            world,
            world_u256,
            world_u512,
            sessions: Mutex::new(SessionState::new(world, world_u256)),
            now,
            society: crate::society::Society::from_env(),
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

/// Build the leaked u128 world once. Walks `crate::SERVICES` and
/// calls `register_u128` on each (default no-op for services that
/// don't use this width). Subsequent calls return the same `&'static`.
fn build_world() -> &'static World<u128> {
    static W: OnceLock<&'static World<u128>> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = World::<u128>::new();
        for svc in crate::SERVICES {
            svc.register_u128(&mut w)
                .unwrap_or_else(|e| panic!("register_u128 for service `{}` failed: {:?}", svc.name(), e));
        }
        Box::leak(Box::new(w))
    })
}

/// Build the parallel U256 world by walking SERVICES.
fn build_world_u256() -> &'static World<U256> {
    static W: OnceLock<&'static World<U256>> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = World::<U256>::new();
        for svc in crate::SERVICES {
            svc.register_u256(&mut w)
                .unwrap_or_else(|e| panic!("register_u256 for service `{}` failed: {:?}", svc.name(), e));
        }
        Box::leak(Box::new(w))
    })
}

/// Build the parallel U512 world by walking SERVICES.
fn build_world_u512() -> &'static World<U512> {
    static W: OnceLock<&'static World<U512>> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = World::<U512>::new();
        for svc in crate::SERVICES {
            svc.register_u512(&mut w)
                .unwrap_or_else(|e| panic!("register_u512 for service `{}` failed: {:?}", svc.name(), e));
        }
        Box::leak(Box::new(w))
    })
}
