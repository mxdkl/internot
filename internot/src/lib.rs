//! `internot` — the procedural world.
//!
//! Services: `directory` (people of the society world: names, family,
//! households, addresses; `docs/superpowers/specs/2026-10-01-directory.md`)
//! and `trace` (the cross-cutting verdict view). The old `people` service
//! was cut over to `directory` on 2026-10-01 (D6).
//!
//! Services plug into the substrate by implementing the [`Service`]
//! trait and being listed in [`SERVICES`]. `Universe::build_world*()`
//! and [`registry()`] both walk that list — they do not name services
//! individually. Adding a service is a one-line edit here.
//!
//! Transports (`internot_mcp`, …) live in separate crates. They build
//! a `Universe` and a `ViewRegistry` at startup and bind every view
//! in the registry to their native idiom (MCP tools, etc.). They
//! contain zero domain knowledge.

pub mod directory;
pub mod services;
pub mod society;
pub mod trace;
pub mod universe;
pub mod views;

pub use services::Service;
pub use universe::{MutationTrace, SessionState, Universe, DEFAULT_NOW};
pub use views::{DynView, View, ViewError, ViewRegistry};

/// The canonical list of services in this build. Single source of
/// truth — `Universe::build_world*()` and [`registry()`] both walk it.
/// Adding a service: implement `Service` on a unit tag struct, then
/// add `&YourService` to this list.
pub const SERVICES: &[&dyn Service] = &[&directory::DirectoryService, &trace::TraceService];

/// Build the canonical view registry by concatenating each service's
/// `views()`. Transports call this once at startup.
pub fn registry() -> ViewRegistry {
    let mut r = ViewRegistry::new();
    for svc in SERVICES {
        r.extend(svc.views());
    }
    r
}
