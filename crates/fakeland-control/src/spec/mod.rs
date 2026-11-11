//! Short text forms of session settings, such as `2560x1440@143.912` or `vrr:48-144`, and the
//! session spec that turns them into a session config. The CLI flags and the MCP session tool
//! share these forms.

mod error;
mod parse;
mod session;
mod window;

pub use error::SpecError;
pub use parse::{
    parse_callbacks, parse_composite, parse_output, parse_renderer, parse_size, parse_timing,
    parse_window_policy,
};
pub use session::SessionSpec;
pub use window::parse_window_ref;
