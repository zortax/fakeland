//! The JSON summary of a trace: per-output vblank health, per-surface frame statistics and input
//! latency.

mod input;
mod output;
mod surface;
mod trace;

pub use input::InputSummary;
pub use output::OutputSummary;
pub use surface::{SurfaceSummary, SyncCounts};
pub use trace::TraceSummary;
