//! Easing functions that map linear progress to eased progress, with the CSS cubic-bezier
//! definitions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::MotionError;

/// An easing function on `0..=1`.
///
/// JSON accepts a name (`"ease_in_out"`) or a tagged object
/// (`{"type": "cubic", "x1": 0.25, "y1": 0.1, "x2": 0.25, "y2": 1.0}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "EasingRepr", into = "EasingRepr")]
pub enum Easing {
    /// Constant speed.
    #[default]
    Linear,
    /// Slow start: `cubic-bezier(0.42, 0, 1, 1)`.
    EaseIn,
    /// Slow end: `cubic-bezier(0, 0, 0.58, 1)`.
    EaseOut,
    /// Slow start and end: `cubic-bezier(0.42, 0, 0.58, 1)`.
    EaseInOut,
    /// A CSS cubic Bézier with control points `(x1, y1)` and `(x2, y2)`.
    Cubic {
        /// First control point x, in `0..=1`.
        x1: f64,
        /// First control point y.
        y1: f64,
        /// Second control point x, in `0..=1`.
        x2: f64,
        /// Second control point y.
        y2: f64,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum EasingRepr {
    Name(EasingName),
    Tagged(EasingTagged),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum EasingName {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EasingTagged {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Cubic { x1: f64, y1: f64, x2: f64, y2: f64 },
}

impl From<EasingRepr> for Easing {
    fn from(r: EasingRepr) -> Self {
        match r {
            EasingRepr::Name(EasingName::Linear) | EasingRepr::Tagged(EasingTagged::Linear) => {
                Self::Linear
            }
            EasingRepr::Name(EasingName::EaseIn) | EasingRepr::Tagged(EasingTagged::EaseIn) => {
                Self::EaseIn
            }
            EasingRepr::Name(EasingName::EaseOut) | EasingRepr::Tagged(EasingTagged::EaseOut) => {
                Self::EaseOut
            }
            EasingRepr::Name(EasingName::EaseInOut)
            | EasingRepr::Tagged(EasingTagged::EaseInOut) => Self::EaseInOut,
            EasingRepr::Tagged(EasingTagged::Cubic { x1, y1, x2, y2 }) => {
                Self::Cubic { x1, y1, x2, y2 }
            }
        }
    }
}

impl From<Easing> for EasingRepr {
    fn from(e: Easing) -> Self {
        match e {
            Easing::Linear => Self::Name(EasingName::Linear),
            Easing::EaseIn => Self::Name(EasingName::EaseIn),
            Easing::EaseOut => Self::Name(EasingName::EaseOut),
            Easing::EaseInOut => Self::Name(EasingName::EaseInOut),
            Easing::Cubic { x1, y1, x2, y2 } => {
                Self::Tagged(EasingTagged::Cubic { x1, y1, x2, y2 })
            }
        }
    }
}

impl Easing {
    /// Returns the control points of the cubic Bézier.
    pub fn control_points(&self) -> (f64, f64, f64, f64) {
        match *self {
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
            Self::EaseIn => (0.42, 0.0, 1.0, 1.0),
            Self::EaseOut => (0.0, 0.0, 0.58, 1.0),
            Self::EaseInOut => (0.42, 0.0, 0.58, 1.0),
            Self::Cubic { x1, y1, x2, y2 } => (x1, y1, x2, y2),
        }
    }

    /// Checks that the easing can be evaluated.
    pub fn validate(&self) -> Result<(), MotionError> {
        let (x1, y1, x2, y2) = self.control_points();
        if ![x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
            return Err(MotionError::NotFinite("easing control point"));
        }
        if !(0.0..=1.0).contains(&x1) || !(0.0..=1.0).contains(&x2) {
            return Err(MotionError::CubicRange { x1, x2 });
        }
        Ok(())
    }

    /// Maps linear progress `t` in `0..=1` to eased progress. `t` outside the range is clamped.
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        if matches!(self, Self::Linear) || t == 0.0 || t == 1.0 {
            return t;
        }
        let (x1, y1, x2, y2) = self.control_points();
        let u = solve_x(t, x1, x2);
        bezier(u, y1, y2)
    }
}

fn bezier(u: f64, p1: f64, p2: f64) -> f64 {
    let v = 1.0 - u;
    3.0 * v * v * u * p1 + 3.0 * v * u * u * p2 + u * u * u
}

fn bezier_slope(u: f64, p1: f64, p2: f64) -> f64 {
    let v = 1.0 - u;
    3.0 * v * v * p1 + 6.0 * v * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
}

/// Finds the curve parameter whose x value is `x`. Newton steps converge fast; bisection covers
/// flat slopes.
fn solve_x(x: f64, x1: f64, x2: f64) -> f64 {
    let mut u = x;
    for _ in 0..8 {
        let err = bezier(u, x1, x2) - x;
        if err.abs() < 1e-9 {
            return u;
        }
        let slope = bezier_slope(u, x1, x2);
        if slope.abs() < 1e-6 {
            break;
        }
        u = (u - err / slope).clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        u = (lo + hi) / 2.0;
        if bezier(u, x1, x2) < x {
            lo = u;
        } else {
            hi = u;
        }
    }
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_fixed() {
        for e in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::Cubic {
                x1: 0.1,
                y1: 1.5,
                x2: 0.9,
                y2: -0.5,
            },
        ] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert_eq!(e.apply(-1.0), 0.0);
            assert_eq!(e.apply(2.0), 1.0);
        }
    }

    #[test]
    fn known_values() {
        assert!((Easing::Linear.apply(0.3) - 0.3).abs() < 1e-12);
        assert!((Easing::EaseInOut.apply(0.5) - 0.5).abs() < 1e-6);
        assert!(Easing::EaseIn.apply(0.25) < 0.25);
        assert!(Easing::EaseOut.apply(0.25) > 0.25);
        // CSS "ease" at x = 0.5 is about 0.8024.
        let ease = Easing::Cubic {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        };
        assert!((ease.apply(0.5) - 0.8024).abs() < 1e-3);
    }

    #[test]
    fn monotonic_for_monotonic_curves() {
        for e in [Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            let mut prev = 0.0;
            for i in 1..=100 {
                let v = e.apply(i as f64 / 100.0);
                assert!(v >= prev);
                prev = v;
            }
        }
    }

    #[test]
    fn serde_accepts_names_and_tags() {
        let e: Easing = serde_json::from_str("\"ease_in_out\"").unwrap_or_default();
        assert_eq!(e, Easing::EaseInOut);
        let e: Easing = serde_json::from_str(r#"{"type":"ease_out"}"#).unwrap_or_default();
        assert_eq!(e, Easing::EaseOut);
        let json = r#"{"type":"cubic","x1":0.1,"y1":0.2,"x2":0.3,"y2":0.4}"#;
        let e: Easing = serde_json::from_str(json).unwrap_or_default();
        assert_eq!(serde_json::to_string(&e).unwrap_or_default(), json);
        assert_eq!(
            serde_json::to_string(&Easing::EaseIn).unwrap_or_default(),
            "\"ease_in\""
        );
    }

    #[test]
    fn validation() {
        assert!(
            Easing::Cubic {
                x1: 1.5,
                y1: 0.0,
                x2: 0.5,
                y2: 1.0
            }
            .validate()
            .is_err()
        );
        assert!(Easing::EaseIn.validate().is_ok());
    }
}
