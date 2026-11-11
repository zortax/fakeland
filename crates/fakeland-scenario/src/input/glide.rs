//! Pointer glides along a path and drags with a held button.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Button;
use crate::{Easing, Path, Position};

fn pointer_rate() -> f64 {
    125.0
}

fn drag_hold() -> Dur {
    Dur::from_millis(50)
}

/// Moves the pointer along a path. Motion events come at `rate_hz` and include both ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PointerGlide {
    /// The path.
    pub path: Path,
    /// The time from the first to the last motion.
    pub duration: Dur,
    /// The easing of the progress along the path.
    #[serde(default)]
    pub easing: Easing,
    /// The motion event rate.
    #[serde(default = "pointer_rate")]
    pub rate_hz: f64,
}

impl PointerGlide {
    /// Builds a linear glide at 125 Hz.
    pub fn new(path: Path, duration: Dur) -> Self {
        Self {
            path,
            duration,
            easing: Easing::Linear,
            rate_hz: pointer_rate(),
        }
    }

    /// Sets the easing.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Sets the event rate.
    pub fn rate(mut self, rate_hz: f64) -> Self {
        self.rate_hz = rate_hz;
        self
    }
}

/// Moves to `from`, presses the button, waits `hold`, glides along `path`, waits `hold` and
/// releases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Drag {
    /// The press position.
    pub from: Position,
    /// The path after the press.
    pub path: Path,
    /// The button.
    #[serde(default)]
    pub button: Button,
    /// The time of the glide.
    pub duration: Dur,
    /// The easing of the glide.
    #[serde(default)]
    pub easing: Easing,
    /// The motion event rate.
    #[serde(default = "pointer_rate")]
    pub rate_hz: f64,
    /// The pause after the press and before the release.
    #[serde(default = "drag_hold")]
    pub hold: Dur,
}

impl Drag {
    /// Builds a straight drag with the left button.
    pub fn line(from: Position, to: Position, duration: Dur) -> Self {
        Self {
            path: Path::line(from.clone(), to),
            from,
            button: Button::Left,
            duration,
            easing: Easing::Linear,
            rate_hz: pointer_rate(),
            hold: drag_hold(),
        }
    }

    /// Sets the button.
    pub fn button(mut self, button: Button) -> Self {
        self.button = button;
        self
    }

    /// Sets the easing.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}
