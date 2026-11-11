//! Parameters that every preset shares, window points and detector choices.

use fakeland_scenario::{Detector, Position, Roi, WindowRef};
use fakeland_types::{Dur, Logical, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use crate::runner::Bench;

/// The parameters every preset reads. A missing value takes the preset default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CommonParams {
    /// The window under test. Default: the focused window.
    pub target: Option<WindowRef>,
    /// The measured iterations per variant.
    pub iterations: Option<u32>,
    /// The warmup iterations per variant.
    pub warmup: Option<u32>,
    /// The pause before each iteration.
    pub settle: Option<Dur>,
    /// The seed of phases, jitter and bootstrap intervals. Default: 0.
    pub seed: Option<u64>,
    /// How long the bench waits for the target window. Default: 10 s.
    pub window_timeout: Option<Dur>,
}

/// The defaults of the shared parameters of one preset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Defaults {
    pub iterations: u32,
    pub warmup: u32,
    pub settle: Dur,
}

impl CommonParams {
    /// Returns the parameters with every default filled in, for listings.
    pub(crate) fn filled(d: Defaults) -> Self {
        Self {
            target: Some(WindowRef::Focused),
            iterations: Some(d.iterations),
            warmup: Some(d.warmup),
            settle: Some(d.settle),
            seed: Some(0),
            window_timeout: Some(Dur::from_secs(10)),
        }
    }

    /// Returns the seed.
    pub(crate) fn seed(&self) -> u64 {
        self.seed.unwrap_or(0)
    }

    /// Applies the shared parameters to a bench.
    pub(crate) fn apply(&self, bench: Bench, d: Defaults, with_target: bool) -> Bench {
        let mut b = bench
            .iterations(self.iterations.unwrap_or(d.iterations))
            .warmup(self.warmup.unwrap_or(d.warmup))
            .settle(self.settle.unwrap_or(d.settle))
            .seed(self.seed())
            .window_timeout(self.window_timeout.unwrap_or(Dur::from_secs(10)));
        if with_target {
            b = b.target(self.target.clone().unwrap_or(WindowRef::Focused));
        }
        b
    }
}

/// A point in a window: fractions of its size, or logical coordinates from its top left corner.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WindowPoint {
    /// Fractions of the window size, such as `{"fx": 0.5, "fy": 0.5}` for the center.
    Fraction {
        /// The horizontal fraction.
        fx: f64,
        /// The vertical fraction.
        fy: f64,
    },
    /// Logical coordinates, such as `{"x": 200, "y": 175}`.
    Logical {
        /// The horizontal coordinate.
        x: f64,
        /// The vertical coordinate.
        y: f64,
    },
}

impl WindowPoint {
    /// The window center.
    pub const CENTER: Self = Self::Fraction { fx: 0.5, fy: 0.5 };

    /// Builds a fraction point.
    pub const fn fraction(fx: f64, fy: f64) -> Self {
        Self::Fraction { fx, fy }
    }

    /// Returns the position in a window.
    pub fn at(&self, window: WindowRef) -> Position {
        match *self {
            Self::Fraction { fx, fy } => Position::fraction(window, fx, fy),
            Self::Logical { x, y } => Position::window(window, x, y),
        }
    }

    /// Returns true for finite coordinates.
    pub fn is_finite(&self) -> bool {
        match *self {
            Self::Fraction { fx, fy } => fx.is_finite() && fy.is_finite(),
            Self::Logical { x, y } => x.is_finite() && y.is_finite(),
        }
    }
}

/// What detects the response to a probe input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DetectorKind {
    /// Pixels with a region, else damage, which costs no readback.
    #[default]
    Auto,
    /// The first presented frame whose region pixels changed.
    Pixels,
    /// The first commit with damage in the region.
    Damage,
    /// The app reports the input as handled through the marks protocol.
    AppMark,
}

impl DetectorKind {
    /// Builds the detector for a window region. `roi: None` covers the window.
    pub fn detector(
        self,
        window: WindowRef,
        roi: Option<Rect<Logical>>,
        threshold: f64,
    ) -> Detector {
        let has_roi = roi.is_some();
        let roi = match roi {
            Some(r) => Roi::window_rect(window, r),
            None => Roi::window(window),
        };
        match self {
            Self::Auto if has_roi => Detector::PixelsChanged { roi, threshold },
            Self::Auto => Detector::AnyDamage { roi },
            Self::Pixels => Detector::PixelsChanged { roi, threshold },
            Self::Damage => Detector::AnyDamage { roi },
            Self::AppMark => Detector::AppMark { name: None },
        }
    }
}

/// Checks that a duration is positive.
pub(crate) fn positive(preset: &str, what: &str, d: Dur) -> Result<(), PresetError> {
    if d.0.as_nanos() > 0 {
        Ok(())
    } else {
        Err(PresetError::params(
            preset,
            format!("{what} must be positive, got {d}"),
        ))
    }
}

/// Checks that a value is finite and positive.
pub(crate) fn positive_f64(preset: &str, what: &str, v: f64) -> Result<(), PresetError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(PresetError::params(
            preset,
            format!("{what} must be a positive number, got {v}"),
        ))
    }
}
