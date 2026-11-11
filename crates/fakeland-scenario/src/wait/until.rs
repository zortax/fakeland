//! Waits. The compositor resolves each wait on its own thread; a wait that times out fails the
//! scenario.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PixelCondition;
use crate::{Roi, WindowRef};

fn default_timeout() -> Dur {
    Dur::from_secs(10)
}

fn default_quiet() -> Dur {
    Dur::from_millis(250)
}

fn default_frames() -> u32 {
    3
}

/// A condition to wait for. JSON tags it with `until`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "until", rename_all = "snake_case")]
pub enum Wait {
    /// A matching window maps.
    Window {
        /// The matcher.
        matcher: WindowRef,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// No commit changes content for `quiet`, counted from the start of the wait at the earliest.
    Settled {
        /// The window. `None` watches every window.
        #[serde(default)]
        window: Option<WindowRef>,
        /// The quiet time.
        #[serde(default = "default_quiet")]
        quiet: Dur,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// `n` frames of the window present.
    Frames {
        /// The frame count.
        n: u32,
        /// The window. `None` counts frames of any window.
        #[serde(default)]
        window: Option<WindowRef>,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// The next frame of the window presents.
    Presented {
        /// The window. `None` waits for any window.
        #[serde(default)]
        window: Option<WindowRef>,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// The region stays unchanged for `frames` presented frames.
    PixelsStable {
        /// The region.
        roi: Roi,
        /// The number of identical frames.
        #[serde(default = "default_frames")]
        frames: u32,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// The region meets a pixel condition.
    PixelsMatch {
        /// The region.
        roi: Roi,
        /// The condition.
        condition: PixelCondition,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
    /// The app sends a `fakeland_marks_v1` mark with this name.
    AppMark {
        /// The mark name.
        name: String,
        /// The deadline.
        #[serde(default = "default_timeout")]
        timeout: Dur,
    },
}

impl Wait {
    /// Waits for a window.
    pub fn window(matcher: WindowRef) -> Self {
        Self::Window {
            matcher,
            timeout: default_timeout(),
        }
    }

    /// Waits until the window settles for `quiet`.
    pub fn settled(window: Option<WindowRef>, quiet: Dur) -> Self {
        Self::Settled {
            window,
            quiet,
            timeout: default_timeout(),
        }
    }

    /// Waits for `n` frames.
    pub fn frames(n: u32, window: Option<WindowRef>) -> Self {
        Self::Frames {
            n,
            window,
            timeout: default_timeout(),
        }
    }

    /// Waits for the next presented frame.
    pub fn presented(window: Option<WindowRef>) -> Self {
        Self::Presented {
            window,
            timeout: default_timeout(),
        }
    }

    /// Waits until the region is stable.
    pub fn pixels_stable(roi: Roi, frames: u32) -> Self {
        Self::PixelsStable {
            roi,
            frames,
            timeout: default_timeout(),
        }
    }

    /// Waits until the region meets a condition.
    pub fn pixels_match(roi: Roi, condition: PixelCondition) -> Self {
        Self::PixelsMatch {
            roi,
            condition,
            timeout: default_timeout(),
        }
    }

    /// Waits for an app mark.
    pub fn app_mark(name: impl Into<String>) -> Self {
        Self::AppMark {
            name: name.into(),
            timeout: default_timeout(),
        }
    }

    /// Returns the deadline of the wait.
    pub fn timeout(&self) -> Dur {
        match self {
            Self::Window { timeout, .. }
            | Self::Settled { timeout, .. }
            | Self::Frames { timeout, .. }
            | Self::Presented { timeout, .. }
            | Self::PixelsStable { timeout, .. }
            | Self::PixelsMatch { timeout, .. }
            | Self::AppMark { timeout, .. } => *timeout,
        }
    }

    /// Replaces the deadline.
    pub fn with_timeout(mut self, new: Dur) -> Self {
        match &mut self {
            Self::Window { timeout, .. }
            | Self::Settled { timeout, .. }
            | Self::Frames { timeout, .. }
            | Self::Presented { timeout, .. }
            | Self::PixelsStable { timeout, .. }
            | Self::PixelsMatch { timeout, .. }
            | Self::AppMark { timeout, .. } => *timeout = new,
        }
        self
    }
}
