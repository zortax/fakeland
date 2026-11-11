//! Trace files: write a trace in any export format with its summary, and read a trace file
//! back.

mod read;
mod write;

pub use read::{ReadError, read_trace};
pub use write::{ExportError, export_trace, span_ms, trace_info};
