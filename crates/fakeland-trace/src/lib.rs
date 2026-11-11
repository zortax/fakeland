//! Trace events on CLOCK_MONOTONIC nanoseconds, the recorder that captures them, the per-surface
//! frame timelines built from them, their statistics, and the exporters that write them as Perfetto
//! JSON, JSONL, CSV and summaries.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod context;
pub mod csv;
pub mod event;
pub mod frames;
pub mod perfetto;
pub mod recorder;
pub mod stats;
pub mod summary;
pub mod trace;

#[cfg(test)]
mod testing;

pub use event::{
    AppMarkKind, BufferInfo, BufferKind, Environment, Event, InputKind, InputSource, OutputInfo,
    Record, SyncKind, TRACE_FORMAT_VERSION,
};
pub use frames::{FrameRecord, FrameTimeline};
pub use perfetto::write_chrome_trace;
pub use recorder::{CaptureId, Recorder, RecorderMode};
pub use stats::Summary;
pub use summary::TraceSummary;
pub use trace::{JsonlWriter, Trace, TraceError, TraceHeader};
