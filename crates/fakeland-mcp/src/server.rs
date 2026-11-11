//! The MCP server: its tool router and its server info.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool_handler};

use crate::state::McpState;

/// The instructions an agent receives at initialization.
const INSTRUCTIONS: &str = "fakeland runs GUI apps in a headless Wayland session that the agent \
controls. Typical flow: launch an app, take a screenshot, act on what you see with click, \
type_text, key, scroll or drag, then take another screenshot. Pointer tools take pixel \
coordinates of the last screenshot by default; set logical=true for logical output coordinates, \
or pass window to use coordinates relative to a window. The first tool call starts a session \
with one 1920x1080 output at 60 Hz; call session_start first for other settings. wait_for waits \
on the compositor for windows, settled content or pixel conditions. measure_latency, \
frame_stats, run_scenario and the trace tools measure apps with compositor timestamps. \
a11y_tree and a11y_find read the accessibility tree of the focused window with element ids, \
and a11y_action and a11y_set_text act on those ids. record_start and record_stop record video \
that follows the display refresh; resources shows CPU, memory and GPU samples of a session \
started with sample.";

/// The fakeland MCP server.
#[derive(Clone)]
pub struct FakelandMcp {
    pub(crate) state: Arc<McpState>,
    tool_router: ToolRouter<Self>,
}

impl FakelandMcp {
    /// Builds the server with every tool.
    pub fn new(state: Arc<McpState>) -> Self {
        Self {
            state,
            tool_router: Self::session_router()
                + Self::screen_router()
                + Self::input_router()
                + Self::measure_router()
                + Self::media_router()
                + Self::a11y_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FakelandMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("fakeland", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
