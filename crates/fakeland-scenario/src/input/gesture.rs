//! Touchpad gestures: pinch, swipe and hold.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Easing, Position};

fn two() -> u32 {
    2
}

fn three() -> u32 {
    3
}

fn unit_scale() -> f64 {
    1.0
}

fn gesture_rate() -> f64 {
    120.0
}

/// A touchpad gesture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Gesture {
    /// Fingers spread or close and may rotate. The pointer moves to `center` first.
    Pinch {
        /// The pointer position.
        center: Position,
        /// The scale at the begin; 1.0 is the begin finger distance.
        #[serde(default = "unit_scale")]
        scale_from: f64,
        /// The scale at the end.
        scale_to: f64,
        /// The total rotation in degrees, clockwise.
        #[serde(default)]
        rotation: f64,
        /// The time from begin to end.
        duration: Dur,
        /// The finger count.
        #[serde(default = "two")]
        fingers: u32,
        /// The easing of the progress.
        #[serde(default)]
        easing: Easing,
        /// The update rate.
        #[serde(default = "gesture_rate")]
        rate_hz: f64,
    },
    /// Fingers move together.
    Swipe {
        /// Moves the pointer here first.
        #[serde(default)]
        at: Option<Position>,
        /// The total horizontal distance in pixels.
        dx: f64,
        /// The total vertical distance in pixels.
        dy: f64,
        /// The time from begin to end.
        duration: Dur,
        /// The finger count.
        #[serde(default = "three")]
        fingers: u32,
        /// The easing of the progress.
        #[serde(default)]
        easing: Easing,
        /// The update rate.
        #[serde(default = "gesture_rate")]
        rate_hz: f64,
    },
    /// Fingers rest on the touchpad.
    Hold {
        /// Moves the pointer here first.
        #[serde(default)]
        at: Option<Position>,
        /// The time from begin to end.
        duration: Dur,
        /// The finger count.
        #[serde(default = "two")]
        fingers: u32,
        /// The hold ends cancelled.
        #[serde(default)]
        cancelled: bool,
    },
}

impl Gesture {
    /// Builds a two-finger pinch without rotation.
    pub fn pinch(center: Position, scale_to: f64, duration: Dur) -> Self {
        Self::Pinch {
            center,
            scale_from: 1.0,
            scale_to,
            rotation: 0.0,
            duration,
            fingers: 2,
            easing: Easing::Linear,
            rate_hz: gesture_rate(),
        }
    }

    /// Builds a three-finger swipe.
    pub fn swipe(dx: f64, dy: f64, duration: Dur) -> Self {
        Self::Swipe {
            at: None,
            dx,
            dy,
            duration,
            fingers: 3,
            easing: Easing::Linear,
            rate_hz: gesture_rate(),
        }
    }

    /// Builds a two-finger hold.
    pub fn hold(duration: Dur) -> Self {
        Self::Hold {
            at: None,
            duration,
            fingers: 2,
            cancelled: false,
        }
    }
}
