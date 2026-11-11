//! Runtime output changes and the public view of an output.

use fakeland_scenario::{OutputAction, Transform};
use fakeland_timing::TimingConfig;
use fakeland_types::{Logical, OutputId, Size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::{ModeConfig, OutputPosition};

/// One change to a running output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputChange {
    /// Switches the current mode. A fixed-rate output follows the new refresh rate.
    Mode(ModeConfig),
    /// Sets the scale.
    Scale {
        /// The scale.
        scale: f64,
    },
    /// Sets the transform.
    Transform {
        /// The transform.
        transform: Transform,
    },
    /// Replaces the timing model.
    Timing(TimingConfig),
    /// Moves the output in the global space.
    Position(OutputPosition),
    /// Powers the output on.
    Enable,
    /// Powers the output off. An off output has no vblanks and holds frame callbacks.
    Disable,
}

impl OutputChange {
    /// Converts a scenario output action into its target and change.
    pub fn from_action(action: &OutputAction) -> (Option<OutputId>, Self) {
        let change = match action {
            OutputAction::SetMode { w, h, refresh, .. } => {
                Self::Mode(ModeConfig::new(*w, *h, *refresh))
            }
            OutputAction::SetScale { scale, .. } => Self::Scale { scale: *scale },
            OutputAction::SetTransform { transform, .. } => Self::Transform {
                transform: *transform,
            },
            OutputAction::SetTiming { timing, .. } => Self::Timing(*timing),
            OutputAction::Enable { .. } => Self::Enable,
            OutputAction::Disable { .. } => Self::Disable,
        };
        (action.output(), change)
    }
}

/// A running output as the session sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OutputState {
    /// The output id.
    pub id: OutputId,
    /// The connector name.
    pub name: String,
    /// The monitor manufacturer.
    pub make: String,
    /// The monitor model.
    pub model: String,
    /// The monitor serial number.
    pub serial: String,
    /// The current mode.
    pub mode: ModeConfig,
    /// Every advertised mode.
    pub modes: Vec<ModeConfig>,
    /// The scale.
    pub scale: f64,
    /// The transform.
    pub transform: Transform,
    /// The position in the global space.
    pub position: OutputPosition,
    /// The logical size.
    pub logical_size: Size<Logical>,
    /// The timing model in effect.
    pub timing: TimingConfig,
    /// The output is powered on.
    pub enabled: bool,
    /// The physical size in millimeters.
    pub physical_mm: (i32, i32),
}
