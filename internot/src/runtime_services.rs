//! `Services` — process-wide infrastructure that lives next to
//! `Universe` but is *not* substrate.
//!
//! Universe owns substrate state (worlds, sessions, simulated time).
//! `Services` owns external integrations (LLM renderer Arcs, future
//! cross-cutting infra like metrics emitters, S3 caches, etc.). The
//! split keeps Universe focused on the procedural floor.
//!
//! Construction model: build once at process startup, attach to
//! Universe via the public `services` field. Tests that don't need
//! renderers leave it `Default::default()` (empty). The MCP transport
//! builds `Services::from_env()` explicitly so external clients +
//! caches are constructed once, not per-call.
//!
//! After the 2026-05-14 nuke, `Services` carries no renderer fields —
//! the mail LLM renderer was deleted alongside its consuming service.
//! Rebuilt services that want LLM rendering will add their renderer
//! Arc here.

#[derive(Default)]
pub struct Services {}

impl Services {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build all renderers that can be constructed from the current
    /// process environment. Currently a no-op — kept on the type so
    /// transports can call it unconditionally (`Services::from_env()`)
    /// without compile-time feature gating.
    pub fn from_env() -> Self {
        Self::default()
    }
}
