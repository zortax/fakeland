//! The scenario executor: runs scenario steps on the compositor thread at their anchors, keeps
//! the per-step timings, probe reports, captures and trigger firings, and returns them with the
//! trace of the run span.

mod anchor;
mod cursor;
mod error;
mod execute;
mod registry;
mod result;

pub use cursor::{Cursor, NextStep};
pub use error::{RunError, RunErrorKind};
pub use registry::{CurrentStep, Run, Runs};
pub use result::{CaptureResult, RunOptions, RunResult, StepTiming, TriggerFiring};
