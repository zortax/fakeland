//! Control operations on a session. Each operation takes the typed parameters of its method and
//! returns its typed result. The socket server and the MCP server both call these.

mod a11y;
mod apps;
mod bench;
mod capture;
mod clipboard;
mod controller;
mod error;
mod input;
mod media;
mod outputs;
mod probes;
mod runs;
mod timer;
mod trace;
mod triggers;
mod windows;

pub use apps::{app_info, describe_exit, tail_lines};
pub use bench::{build_bench, preset_entries, read_report};
pub use capture::{encode_png, shot_info, shot_meta, write_file};
pub use controller::Controller;
pub use error::OpError;
pub use input::act_outcome;
pub use runs::{RunFilesRef, RunNotify, summarize};
pub use timer::with_timeout;
