//! Generic registry-walking MCP server.
//!
//! Holds an `internot::Universe` and an `internot::ViewRegistry`. The
//! `ServerHandler` impl is hand-written (not via macros) because the
//! tool list is built at runtime from the registry, not at compile
//! time from `#[tool]` annotations. Every view in the registry
//! becomes an MCP tool automatically.

use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};

use internot::{Universe, ViewRegistry};

pub struct InternotServer {
    universe: Arc<Universe>,
    registry: Arc<ViewRegistry>,
}

impl InternotServer {
    pub fn new() -> Self {
        // Builds the society world of `INTERNOT_PACK` (default `us`) once.
        let universe = Universe::new();
        InternotServer {
            universe: Arc::new(universe),
            registry: Arc::new(internot::registry()),
        }
    }

    pub fn view_count(&self) -> usize {
        self.registry.len()
    }
}

impl ServerHandler for InternotServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::V_2024_11_05;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(
            "Procedural world of people (internot society): read_person and read_household \
             give names, family, unions, households and addresses at any time (`at`, ISO \
             8601). Follow person ids to relatives. Time is 2025-06-30 09:00 UTC by default."
                .to_string(),
        );
        info
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let registry = Arc::clone(&self.registry);
        async move {
            let tools: Vec<Tool> = registry
                .iter()
                .map(|view| {
                    let schema = view.input_schema();
                    let schema_obj = match schema {
                        serde_json::Value::Object(map) => map,
                        _ => serde_json::Map::new(),
                    };
                    let mut tool = Tool::default();
                    tool.name = view.name().to_string().into();
                    tool.description = Some(view.description().to_string().into());
                    tool.input_schema = Arc::new(schema_obj);
                    tool
                })
                .collect();
            // ListToolsResult is #[non_exhaustive] so the
            // struct-update syntax doesn't apply; build via Default
            // then mutate. Allow the lint.
            #[allow(clippy::field_reassign_with_default)]
            let result = {
                let mut r = ListToolsResult::default();
                r.tools = tools;
                r
            };
            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let registry = Arc::clone(&self.registry);
        let universe = Arc::clone(&self.universe);
        async move {
            let name = request.name.as_ref();
            let view = registry.get(name).ok_or_else(|| {
                McpError::invalid_params(format!("unknown tool: {}", name), None)
            })?;
            let raw = match request.arguments {
                Some(map) => serde_json::Value::Object(map),
                None => serde_json::Value::Object(serde_json::Map::new()),
            };
            match view.execute(&universe, raw) {
                Ok(out) => {
                    let text = serde_json::to_string_pretty(&out)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(e.to_string(), None)),
            }
        }
    }
}
