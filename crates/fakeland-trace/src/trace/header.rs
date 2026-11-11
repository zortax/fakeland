//! The trace header: format version, host environment, outputs and start time.

use fakeland_types::Timestamp;
use serde::{Deserialize, Serialize};

use crate::event::{Environment, OutputInfo, TRACE_FORMAT_VERSION};
use crate::trace::TraceError;

/// The first part of every trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceHeader {
    /// The trace format version.
    pub format_version: u32,
    /// The host environment.
    pub environment: Environment,
    /// The outputs at the start of the trace.
    pub outputs: Vec<OutputInfo>,
    /// The start of the trace.
    pub started: Timestamp,
    /// The number of records that a bounded recorder dropped from the start of the trace.
    #[serde(default)]
    pub dropped: u64,
    /// True when the session shows its outputs in a nested viewer window. The viewer adds GPU
    /// and compositor load.
    #[serde(default)]
    pub viewer: bool,
}

impl TraceHeader {
    /// Builds a header of the current format version.
    pub fn new(environment: Environment, outputs: Vec<OutputInfo>, started: Timestamp) -> Self {
        Self {
            format_version: TRACE_FORMAT_VERSION,
            environment,
            outputs,
            started,
            dropped: 0,
            viewer: false,
        }
    }

    /// Returns an error when the header has a format version this build cannot read.
    pub fn check_version(&self) -> Result<(), TraceError> {
        if self.format_version == TRACE_FORMAT_VERSION {
            Ok(())
        } else {
            Err(TraceError::Version {
                found: self.format_version,
                expected: TRACE_FORMAT_VERSION,
            })
        }
    }

    /// Updates the output list from an output event.
    pub fn apply_output(&mut self, info: &OutputInfo) {
        match self.outputs.iter_mut().find(|o| o.output == info.output) {
            Some(o) => *o = info.clone(),
            None => self.outputs.push(info.clone()),
        }
    }
}
