//! Cross-cutting view: `_get_trace`.
//!
//! Returns the cumulative `MutationTrace` for this Universe. The
//! underscore prefix signals "scenario verdict only" — an LLM agent
//! shouldn't normally call this. Lives at the crate root because it
//! spans all services.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::universe::{MutationTrace, Universe};
use crate::views::{DynView, View, ViewError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTraceParams {}

pub struct GetTrace;

impl View for GetTrace {
    type Params = GetTraceParams;
    type Output = MutationTrace;
    const NAME: &'static str = "_get_trace";
    const DESCRIPTION: &'static str =
        "Internal: return the cumulative trace of every mutation performed in this MCP session. Scenario verdicts call this; an LLM agent shouldn't.";

    fn execute(&self, ctx: &Universe, _: GetTraceParams) -> Result<MutationTrace, ViewError> {
        Ok(ctx.sessions.lock().trace.clone())
    }
}

pub fn views() -> Vec<Arc<dyn DynView>> {
    vec![Arc::new(GetTrace)]
}

pub struct TraceService;

impl crate::services::Service for TraceService {
    fn name(&self) -> &'static str { "trace" }
    fn views(&self) -> Vec<Arc<dyn DynView>> { views() }
}
