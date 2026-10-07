//! `directory` — people of the society world through MCP: who someone is,
//! when they lived, their partner and their family, at any time.
//!
//! Read-only. Every answer is a pure function of the world's pack and seed,
//! the person id and the time asked about (`at`, default the Universe's
//! `now`). Spec: `docs/superpowers/specs/2026-10-01-directory.md`.

pub mod views;

use std::sync::Arc;

use crate::services::Service;
use crate::views::DynView;

/// The directory service's tag.
pub struct DirectoryService;

impl Service for DirectoryService {
    fn name(&self) -> &'static str {
        "directory"
    }

    fn views(&self) -> Vec<Arc<dyn DynView>> {
        views::views()
    }
}
