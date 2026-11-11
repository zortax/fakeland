//! Output changes: mode, scale, transform, timing and power.

use fakeland_timing::TimingConfig;
use fakeland_types::{Hz, OutputId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An output transform with the `wl_output.transform` meanings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Transform {
    /// No transform.
    #[default]
    #[serde(rename = "normal")]
    Normal,
    /// Rotated 90 degrees counterclockwise.
    #[serde(rename = "90")]
    Rotate90,
    /// Rotated 180 degrees.
    #[serde(rename = "180")]
    Rotate180,
    /// Rotated 270 degrees counterclockwise.
    #[serde(rename = "270")]
    Rotate270,
    /// Flipped around the vertical axis.
    #[serde(rename = "flipped")]
    Flipped,
    /// Flipped, then rotated 90 degrees.
    #[serde(rename = "flipped_90")]
    Flipped90,
    /// Flipped, then rotated 180 degrees.
    #[serde(rename = "flipped_180")]
    Flipped180,
    /// Flipped, then rotated 270 degrees.
    #[serde(rename = "flipped_270")]
    Flipped270,
}

impl Transform {
    /// Returns the `wl_output.transform` value.
    pub fn wayland_value(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Rotate90 => 1,
            Self::Rotate180 => 2,
            Self::Rotate270 => 3,
            Self::Flipped => 4,
            Self::Flipped90 => 5,
            Self::Flipped180 => 6,
            Self::Flipped270 => 7,
        }
    }
}

/// A change to an output. `output: None` selects the first output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OutputAction {
    /// Switches the mode.
    SetMode {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
        /// Width in pixels.
        w: i32,
        /// Height in pixels.
        h: i32,
        /// The refresh rate.
        refresh: Hz,
    },
    /// Sets the scale; fractional scales are allowed.
    SetScale {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
        /// The scale.
        scale: f64,
    },
    /// Sets the transform.
    SetTransform {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
        /// The transform.
        transform: Transform,
    },
    /// Replaces the display timing model.
    SetTiming {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
        /// The timing configuration.
        timing: TimingConfig,
    },
    /// Turns the output on.
    Enable {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
    },
    /// Turns the output off: no vblanks, held frame callbacks.
    Disable {
        /// The output.
        #[serde(default)]
        output: Option<OutputId>,
    },
}

impl OutputAction {
    /// Returns the output that the change selects.
    pub fn output(&self) -> Option<OutputId> {
        match self {
            Self::SetMode { output, .. }
            | Self::SetScale { output, .. }
            | Self::SetTransform { output, .. }
            | Self::SetTiming { output, .. }
            | Self::Enable { output }
            | Self::Disable { output } => *output,
        }
    }
}
