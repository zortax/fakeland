//! Capture targets of screenshots.

use fakeland_types::OutputId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Roi, WindowRef};

/// What a capture reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaptureTarget {
    /// A whole output.
    Output {
        /// The output. `None` selects the first output.
        #[serde(default)]
        output: Option<OutputId>,
    },
    /// A window and its surface tree.
    Window {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
    /// A region.
    Roi {
        /// The region.
        roi: Roi,
    },
}

impl Default for CaptureTarget {
    fn default() -> Self {
        Self::Output { output: None }
    }
}
