//! `internot-mcp` — generic MCP stdio transport.
//!
//! Walks the `internot::registry()` once at startup and exposes every
//! view as an MCP tool by its `View::NAME`. Contains zero domain
//! knowledge — adding a new view in `internot/` makes it appear here
//! the next time the binary is built.

mod server;

use anyhow::Result;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use crate::server::InternotServer;

fn main() -> Result<()> {
    // MCP transport is JSON-RPC on stdin/stdout — logs MUST go to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    // Build the server (and its world, which takes seconds) before
    // entering the tokio runtime.
    let server = InternotServer::new();
    tracing::info!(
        "internot-mcp ready: {} views registered",
        server.view_count()
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let service = server.serve(stdio()).await?;
        service.waiting().await?;
        Result::<()>::Ok(())
    })
}
