//! Regions of interest for captures, waits, triggers and probes.

use fakeland_types::{Logical, OutputId, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::WindowRef;

/// A region on screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Roi {
    /// A window, or a rectangle in window-relative logical coordinates.
    Window {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// The rectangle. `None` covers the whole window.
        #[serde(default)]
        rect: Option<Rect<Logical>>,
    },
    /// An output, or a rectangle in output-relative logical coordinates.
    Output {
        /// The output. `None` selects the first output.
        #[serde(default)]
        output: Option<OutputId>,
        /// The rectangle. `None` covers the whole output.
        #[serde(default)]
        rect: Option<Rect<Logical>>,
    },
}

impl Roi {
    /// Covers a whole window.
    pub fn window(window: impl Into<WindowRef>) -> Self {
        Self::Window {
            window: window.into(),
            rect: None,
        }
    }

    /// Covers a rectangle of a window.
    pub fn window_rect(window: impl Into<WindowRef>, rect: Rect<Logical>) -> Self {
        Self::Window {
            window: window.into(),
            rect: Some(rect),
        }
    }

    /// Covers a whole output.
    pub fn output(output: Option<OutputId>) -> Self {
        Self::Output { output, rect: None }
    }

    /// Covers a rectangle of an output.
    pub fn output_rect(output: Option<OutputId>, rect: Rect<Logical>) -> Self {
        Self::Output {
            output,
            rect: Some(rect),
        }
    }
}

impl Default for Roi {
    fn default() -> Self {
        Self::window(WindowRef::Focused)
    }
}
