//! MCP server over stdio. It holds a fakeland session in process and gives an agent tools to launch
//! apps, look at them and drive them.
//!
//! The session starts on the first tool call with one 1920x1080 output at 60 Hz, or with the
//! settings of `session_start`. Pointer tools take pixel coordinates of the last screenshot, so an
//! agent can click what it sees. Every measurement uses compositor timestamps.
//!
//! # Setup in Claude Code
//!
//! ```text
//! cargo install --path crates/fakeland-mcp
//! claude mcp add fakeland -- fakeland-mcp
//! ```
//!
//! Or in `.mcp.json`:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "fakeland": { "command": "fakeland-mcp", "args": [] }
//!   }
//! }
//! ```
//!
//! Logs go to stderr; set `FAKELAND_LOG=debug` for more. The session and its apps end when the
//! client closes stdin.

#![forbid(unsafe_code)]

mod coords;
mod params;
mod reply;
mod server;
mod state;
mod tools;

use std::sync::Arc;

use anyhow::Result;
use rmcp::ServiceExt;

use server::FakelandMcp;
use state::McpState;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("FAKELAND_LOG").unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("warn,smithay::backend::drm=error")
            }),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let state = Arc::new(McpState::default());
    let service = FakelandMcp::new(Arc::clone(&state))
        .serve(rmcp::transport::stdio())
        .await?;
    let reason = service.waiting().await;
    tracing::debug!("the MCP client left: {reason:?}");
    state.stop().await;
    Ok(())
}
