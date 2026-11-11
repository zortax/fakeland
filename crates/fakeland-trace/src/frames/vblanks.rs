//! Vblanks of an output.

use fakeland_types::{Nanos, OutputId, Timestamp};
use serde::{Deserialize, Serialize};

/// One vblank of the display clock model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VblankInfo {
    /// The output.
    pub output: OutputId,
    /// The sequence number.
    pub seq: u64,
    /// The ideal time on the model grid.
    pub ideal: Timestamp,
    /// The wake time of the compositor thread.
    pub woke: Timestamp,
    /// True when the perturbation model drops the vblank.
    pub dropped: bool,
}

impl VblankInfo {
    /// Returns how late the compositor thread woke.
    pub fn lateness(&self) -> Nanos {
        self.woke - self.ideal
    }
}
