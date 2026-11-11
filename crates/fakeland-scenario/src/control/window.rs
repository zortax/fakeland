//! Window management actions: size, resize storms, focus, state and close.

use fakeland_types::{Dur, Logical, Rect, Size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Easing, WindowRef};

fn resize_rate() -> f64 {
    60.0
}

/// A window management action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WindowAction {
    /// Configures the window with a new size.
    Resize {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// The logical size.
        size: Size<Logical>,
    },
    /// Configures a stream of sizes from `from` to `to`, like an interactive resize.
    ResizeStorm {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// The first size.
        from: Size<Logical>,
        /// The last size.
        to: Size<Logical>,
        /// The time from the first to the last configure.
        duration: Dur,
        /// The configure rate.
        #[serde(default = "resize_rate")]
        rate_hz: f64,
        /// The easing of the size change.
        #[serde(default)]
        easing: Easing,
    },
    /// Gives the window keyboard focus and activates it.
    Focus {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
    /// Maximizes the window.
    Maximize {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
    /// Makes the window fullscreen.
    Fullscreen {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
    /// Makes the window float with a geometry in output logical coordinates.
    Floating {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// The geometry.
        rect: Rect<Logical>,
    },
    /// Sets or clears the `suspended` toplevel state.
    Suspend {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// The state.
        suspended: bool,
    },
    /// Asks the window to close.
    Close {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
}

impl WindowAction {
    /// Returns the window that the action selects.
    pub fn window(&self) -> &WindowRef {
        match self {
            Self::Resize { window, .. }
            | Self::ResizeStorm { window, .. }
            | Self::Focus { window }
            | Self::Maximize { window }
            | Self::Fullscreen { window }
            | Self::Floating { window, .. }
            | Self::Suspend { window, .. }
            | Self::Close { window } => window,
        }
    }
}
