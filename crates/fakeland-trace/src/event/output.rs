//! The configuration of a faked output.

use fakeland_types::{Hz, OutputId};
use serde::{Deserialize, Serialize};

/// The mode, scale and timing of one output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputInfo {
    /// The output.
    pub output: OutputId,
    /// The connector name, for example `DP-1`.
    pub name: String,
    /// The mode width in physical pixels.
    pub width: i32,
    /// The mode height in physical pixels.
    pub height: i32,
    /// The refresh rate in millihertz. Zero means the output has no fixed refresh.
    pub refresh_mhz: u32,
    /// The output scale.
    pub scale: f64,
    /// The output transform, for example `normal` or `90`.
    pub transform: String,
    /// A description of the display clock model, for example `fixed 143.912 Hz latch 1.5 ms`.
    pub timing: String,
}

impl OutputInfo {
    /// Returns the refresh rate, or `None` when the output has no fixed refresh.
    pub fn refresh(&self) -> Option<Hz> {
        (self.refresh_mhz > 0).then(|| Hz(f64::from(self.refresh_mhz) / 1000.0))
    }
}
