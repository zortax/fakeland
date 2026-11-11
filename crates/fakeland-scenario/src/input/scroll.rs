//! Scroll sources, scroll glides that follow a velocity curve, and flings with decaying velocity.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Position, VelocityCurve};

fn yes() -> bool {
    true
}

fn detent() -> f64 {
    15.0
}

fn scroll_rate() -> f64 {
    120.0
}

fn fling_time_constant() -> Dur {
    Dur::from_millis(325)
}

fn fling_duration() -> Dur {
    Dur::from_secs(1)
}

/// The device that produces axis events.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScrollSource {
    /// A mouse wheel. A detent is 120 `value120` units and `detent` pixels.
    Wheel {
        /// High-resolution wheel: fractional detents. Without it, only whole detents send events.
        #[serde(default = "yes")]
        hires: bool,
        /// The axis distance of one detent in pixels.
        #[serde(default = "detent")]
        detent: f64,
    },
    /// Two fingers on a touchpad: continuous distances, then `axis_stop`.
    Finger {
        /// The stop follows the last motion at once, so the client may continue with kinetic
        /// scrolling. Without it, the stop comes after a pause, so the client sees no velocity.
        #[serde(default = "yes")]
        kinetic: bool,
    },
    /// A continuous device such as a trackpoint: distances without a stop.
    Continuous,
}

impl Default for ScrollSource {
    fn default() -> Self {
        Self::wheel(true)
    }
}

impl ScrollSource {
    /// A wheel with the default detent.
    pub fn wheel(hires: bool) -> Self {
        Self::Wheel {
            hires,
            detent: detent(),
        }
    }

    /// Two fingers on a touchpad.
    pub fn finger(kinetic: bool) -> Self {
        Self::Finger { kinetic }
    }
}

/// The scroll axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScrollAxis {
    /// Up and down. Positive distances scroll down.
    #[default]
    Vertical,
    /// Left and right. Positive distances scroll right.
    Horizontal,
}

/// Scrolls along a velocity curve. The events integrate the curve, so the total distance equals
/// its integral (to the nearest detent for low-resolution wheels).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScrollGlide {
    /// Moves the pointer here first.
    #[serde(default)]
    pub at: Option<Position>,
    /// The axis.
    #[serde(default)]
    pub axis: ScrollAxis,
    /// The velocity over the glide.
    pub velocity: VelocityCurve,
    /// The glide time.
    pub duration: Dur,
    /// The device.
    #[serde(default)]
    pub source: ScrollSource,
    /// The event rate.
    #[serde(default = "scroll_rate")]
    pub rate_hz: f64,
}

impl ScrollGlide {
    /// Builds a vertical glide with a high-resolution wheel.
    pub fn new(velocity: VelocityCurve, duration: Dur) -> Self {
        Self {
            at: None,
            axis: ScrollAxis::Vertical,
            velocity,
            duration,
            source: ScrollSource::default(),
            rate_hz: scroll_rate(),
        }
    }

    /// Builds a vertical glide at a constant velocity.
    pub fn constant(px_per_s: f64, duration: Dur) -> Self {
        Self::new(VelocityCurve::constant(px_per_s), duration)
    }

    /// Sets the pointer position.
    pub fn at(mut self, pos: Position) -> Self {
        self.at = Some(pos);
        self
    }

    /// Sets the axis.
    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Sets the device.
    pub fn source(mut self, source: ScrollSource) -> Self {
        self.source = source;
        self
    }

    /// Sets the event rate.
    pub fn rate(mut self, rate_hz: f64) -> Self {
        self.rate_hz = rate_hz;
        self
    }
}

/// A finger scroll whose velocity decays as `v(t) = velocity * exp(-t / time_constant)`, then
/// `axis_stop`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fling {
    /// Moves the pointer here first.
    #[serde(default)]
    pub at: Option<Position>,
    /// The axis.
    #[serde(default)]
    pub axis: ScrollAxis,
    /// The initial velocity in pixels per second.
    pub velocity: f64,
    /// The decay time constant.
    #[serde(default = "fling_time_constant")]
    pub time_constant: Dur,
    /// The time from the first event to the stop.
    #[serde(default = "fling_duration")]
    pub duration: Dur,
    /// The event rate.
    #[serde(default = "scroll_rate")]
    pub rate_hz: f64,
}

impl Fling {
    /// Builds a vertical fling.
    pub fn new(velocity: f64) -> Self {
        Self {
            at: None,
            axis: ScrollAxis::Vertical,
            velocity,
            time_constant: fling_time_constant(),
            duration: fling_duration(),
            rate_hz: scroll_rate(),
        }
    }

    /// Returns the distance after `t_s` seconds.
    pub fn displacement_at(&self, t_s: f64) -> f64 {
        let tau = self.time_constant.0.as_secs_f64();
        if tau <= 0.0 {
            return 0.0;
        }
        self.velocity * tau * (1.0 - (-t_s / tau).exp())
    }
}
