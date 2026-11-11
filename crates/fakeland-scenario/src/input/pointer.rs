//! Pointer actions: motion, buttons, clicks and single axis events.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Button, ScrollSource};
use crate::Position;

fn one() -> u32 {
    1
}

fn click_interval() -> Dur {
    Dur::from_millis(120)
}

fn click_hold() -> Dur {
    Dur::from_millis(40)
}

/// A pointer action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PointerAction {
    /// Moves the pointer to a position in one motion event.
    MoveTo {
        /// The target.
        pos: Position,
    },
    /// Presses or releases a button.
    Button {
        /// The button.
        #[serde(default)]
        button: Button,
        /// True presses, false releases.
        pressed: bool,
    },
    /// Clicks `count` times. Each click presses at `interval` steps and releases after `hold`.
    Click {
        /// Moves the pointer here first.
        #[serde(default)]
        at: Option<Position>,
        /// The button.
        #[serde(default)]
        button: Button,
        /// The number of clicks: 2 is a double click.
        #[serde(default = "one")]
        count: u32,
        /// The time between presses.
        #[serde(default = "click_interval")]
        interval: Dur,
        /// The time each press lasts. It stays below half the interval.
        #[serde(default = "click_hold")]
        hold: Dur,
    },
    /// One axis event.
    Axis {
        /// Horizontal scroll distance in pixels. Positive scrolls right.
        #[serde(default)]
        dx: f64,
        /// Vertical scroll distance in pixels. Positive scrolls down.
        #[serde(default)]
        dy: f64,
        /// The axis source.
        #[serde(default)]
        source: ScrollSource,
        /// High-resolution wheel steps `[x, y]`; 120 is one detent. `None` derives them from the
        /// distance for wheel sources.
        #[serde(default)]
        value120: Option<(i32, i32)>,
    },
}

impl PointerAction {
    /// Moves the pointer.
    pub fn move_to(pos: Position) -> Self {
        Self::MoveTo { pos }
    }

    /// Presses or releases a button.
    pub fn button(button: Button, pressed: bool) -> Self {
        Self::Button { button, pressed }
    }

    /// Clicks once with a button, optionally at a position.
    pub fn click(at: Option<Position>, button: Button) -> Self {
        Self::Click {
            at,
            button,
            count: 1,
            interval: click_interval(),
            hold: click_hold(),
        }
    }

    /// Clicks `count` times with a button at a position.
    pub fn multi_click(at: Option<Position>, button: Button, count: u32) -> Self {
        match Self::click(at, button) {
            Self::Click {
                at,
                button,
                interval,
                hold,
                ..
            } => Self::Click {
                at,
                button,
                count,
                interval,
                hold,
            },
            other => other,
        }
    }

    /// Scrolls once.
    pub fn axis(dx: f64, dy: f64, source: ScrollSource) -> Self {
        Self::Axis {
            dx,
            dy,
            source,
            value120: None,
        }
    }
}
