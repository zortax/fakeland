//! Touch actions: single touch points, taps and touch glides.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Easing, Path, Position};

fn tap_hold() -> Dur {
    Dur::from_millis(50)
}

fn touch_rate() -> f64 {
    120.0
}

/// A touch action. `id` names the touch point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TouchAction {
    /// A finger touches down.
    Down {
        /// The touch point.
        #[serde(default)]
        id: u32,
        /// The position.
        pos: Position,
    },
    /// A finger moves.
    Motion {
        /// The touch point.
        #[serde(default)]
        id: u32,
        /// The position.
        pos: Position,
    },
    /// A finger lifts.
    Up {
        /// The touch point.
        #[serde(default)]
        id: u32,
    },
    /// A finger touches down and lifts after `hold`.
    Tap {
        /// The touch point.
        #[serde(default)]
        id: u32,
        /// The position.
        pos: Position,
        /// The contact time.
        #[serde(default = "tap_hold")]
        hold: Dur,
    },
    /// A finger touches down at the path start, follows the path and lifts at its end.
    Glide {
        /// The touch point.
        #[serde(default)]
        id: u32,
        /// The path.
        path: Box<Path>,
        /// The time from the first to the last motion.
        duration: Dur,
        /// The easing of the progress along the path.
        #[serde(default)]
        easing: Easing,
        /// The motion event rate.
        #[serde(default = "touch_rate")]
        rate_hz: f64,
    },
}

impl TouchAction {
    /// Taps at a position.
    pub fn tap(pos: Position) -> Self {
        Self::Tap {
            id: 0,
            pos,
            hold: tap_hold(),
        }
    }

    /// Glides a finger along a path.
    pub fn glide(path: Path, duration: Dur) -> Self {
        Self::Glide {
            id: 0,
            path: Box::new(path),
            duration,
            easing: Easing::Linear,
            rate_hz: touch_rate(),
        }
    }
}
