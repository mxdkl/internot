//! `internot` — the procedural world.
//!
//! Services: `directory` (people of the society world: life dates,
//! partners and family; `docs/superpowers/specs/2026-10-01-directory.md`)
//! and `trace` (the cross-cutting verdict view). The society world is the
//! monotone world (`internot_society::mono`) since 2026-10-03.
//!
//! Services plug into the substrate by implementing the [`Service`]
//! trait and being listed in [`SERVICES`]. [`registry()`] walks that
//! list; it does not name services individually. Adding a service is a
//! one-line edit here.
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
/// truth — [`registry()`] walks it.
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
