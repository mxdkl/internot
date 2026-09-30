//! `Service` — the contract every service in this crate satisfies.
//!
//! A service owns a procedural domain (its bit layout, hash-derived
//! attributes, AVM, and views). It plugs into the substrate by
//! implementing this trait on a unit struct and adding that struct
//! to the central [`SERVICES`] list.
//!
//! Adding a new service is a one-line edit to `SERVICES` in
//! `lib.rs`. `Universe::build_world*()` and `crate::registry()`
//! both walk that list — they do not name services individually.
//!
//! The trait has default no-op implementations for every method,
//! so a service only declares what it actually has (e.g., a
//! u128-only service implements `register_u128` + `views` and
//! ignores the U256 / U512 hooks).
//!
//! `Send + Sync + 'static` is required because the SERVICES list
//! is a `&'static [&'static dyn Service]`.

use std::sync::Arc;

use procedural_core::word::{U256, U512};
use procedural_core::world::{World, WorldError};

use crate::views::DynView;

pub trait Service: Send + Sync + 'static {
    /// Service tag used in panic messages and diagnostics. Must be
    /// unique across the SERVICES list (not enforced — convention).
    fn name(&self) -> &'static str;

    /// Register this service's u128 spaces on the shared `World<u128>`.
    /// Default: no-op (service uses U256/U512 instead, or has no
    /// substrate at all).
    fn register_u128(&self, _world: &mut World<u128>) -> Result<(), WorldError> {
        Ok(())
    }

    /// Register this service's U256 spaces. Default: no-op.
    fn register_u256(&self, _world: &mut World<U256>) -> Result<(), WorldError> {
        Ok(())
    }

    /// Register this service's U512 spaces. Default: no-op.
    fn register_u512(&self, _world: &mut World<U512>) -> Result<(), WorldError> {
        Ok(())
    }

    /// Views this service exposes through `crate::registry()`.
    /// Default: none (a substrate-only service with no MCP surface).
    fn views(&self) -> Vec<Arc<dyn DynView>> {
        Vec::new()
    }
}
