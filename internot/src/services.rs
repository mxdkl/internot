//! `Service` — the contract every service in this crate satisfies.
//!
//! A service is a lens on the society world: views over it (and, once
//! services mutate again, a session). It plugs in by implementing this
//! trait on a unit struct and adding that struct to the central
//! [`SERVICES`] list; `crate::registry()` walks that list, never naming
//! services individually.
//!
//! `Send + Sync + 'static` is required because the SERVICES list
//! is a `&'static [&'static dyn Service]`.

use std::sync::Arc;

use crate::views::DynView;

pub trait Service: Send + Sync + 'static {
    /// Service tag used in panic messages and diagnostics. Must be
    /// unique across the SERVICES list (not enforced — convention).
    fn name(&self) -> &'static str;

    /// Views this service exposes through `crate::registry()`.
    /// Default: none (a substrate-only service with no MCP surface).
    fn views(&self) -> Vec<Arc<dyn DynView>> {
        Vec::new()
    }
}
