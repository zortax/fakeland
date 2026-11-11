//! Session configuration. Every type is serde and JSON Schema, so the CLI, the control socket
//! and MCP pass the same config as Rust code.

mod error;
mod input;
mod output;
mod policy;
mod sampling;
mod session;

pub use error::ConfigError;
pub use input::{InputConfig, Keymap, PointerDevice, Repeat, UnmappedKeys, WheelKind};
pub use output::{ModeConfig, OutputConfig, OutputPosition, PhysicalSize, logical_size};
pub use policy::{Composite, DbusMode, Decorations, RendererChoice, WindowPolicy};
pub use sampling::{MIN_SAMPLE_INTERVAL, SamplingConfig};
pub use session::{SessionConfig, validate_output};
