//! Scroll velocity curves over the progress of a glide, and their exact integrals.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::MotionError;

/// A scroll velocity in pixels per second over progress `0..=1`. Positive values scroll down or
/// right.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VelocityCurve {
    /// The same velocity for the whole glide.
    Constant {
        /// The velocity.
        px_per_s: f64,
    },
    /// A linear change from one velocity to another.
    Ramp {
        /// The velocity at the start.
        from: f64,
        /// The velocity at the end.
        to: f64,
    },
    /// Linear interpolation between keyframes `[progress, px_per_s]`, held flat before the first
    /// and after the last.
    Keyframes {
        /// The keyframes, sorted by progress in `0..=1`.
        points: Vec<(f64, f64)>,
    },
}

impl Default for VelocityCurve {
    fn default() -> Self {
        Self::Constant { px_per_s: 1000.0 }
    }
}

impl VelocityCurve {
    /// Builds a constant velocity.
    pub fn constant(px_per_s: f64) -> Self {
        Self::Constant { px_per_s }
    }

    /// Builds a linear ramp.
    pub fn ramp(from: f64, to: f64) -> Self {
        Self::Ramp { from, to }
    }

    /// Builds keyframes.
    pub fn keyframes(points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        Self::Keyframes {
            points: points.into_iter().collect(),
        }
    }

    /// Checks that the curve can be evaluated.
    pub fn validate(&self) -> Result<(), MotionError> {
        match self {
            Self::Constant { px_per_s } => finite(*px_per_s),
            Self::Ramp { from, to } => finite(*from).and(finite(*to)),
            Self::Keyframes { points } => {
                let sorted = points.windows(2).all(|w| w[0].0 <= w[1].0);
                let in_range = points
                    .iter()
                    .all(|(t, v)| (0.0..=1.0).contains(t) && v.is_finite());
                if points.is_empty() || !sorted || !in_range {
                    return Err(MotionError::Keyframes);
                }
                Ok(())
            }
        }
    }

    /// Returns the velocity at progress `f`, clamped to `0..=1`.
    pub fn velocity_at(&self, f: f64) -> f64 {
        let f = f.clamp(0.0, 1.0);
        match self {
            Self::Constant { px_per_s } => *px_per_s,
            Self::Ramp { from, to } => from + (to - from) * f,
            Self::Keyframes { points } => self.knots(points, |a, b| {
                if f <= a.0 {
                    Some(a.1)
                } else if f <= b.0 {
                    let span = b.0 - a.0;
                    Some(if span > 0.0 {
                        a.1 + (b.1 - a.1) * (f - a.0) / span
                    } else {
                        b.1
                    })
                } else {
                    None
                }
            }),
        }
    }

    /// Returns the displacement in pixels after progress `f` of a glide that lasts `duration_s`
    /// seconds: the exact integral of the velocity.
    pub fn displacement_at(&self, f: f64, duration_s: f64) -> f64 {
        let f = f.clamp(0.0, 1.0);
        let integral = match self {
            Self::Constant { px_per_s } => px_per_s * f,
            Self::Ramp { from, to } => from * f + (to - from) * f * f / 2.0,
            Self::Keyframes { points } => {
                let mut knots: Vec<(f64, f64)> = Vec::with_capacity(points.len() + 2);
                let first = points.first().copied().unwrap_or((0.0, 0.0));
                let last = points.last().copied().unwrap_or((1.0, 0.0));
                knots.push((0.0, first.1));
                knots.extend(points.iter().copied());
                knots.push((1.0, last.1));
                let mut area = 0.0;
                for w in knots.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    if f <= a.0 {
                        break;
                    }
                    let end = f.min(b.0);
                    let v_end = self.velocity_at(end);
                    area += (a.1 + v_end) / 2.0 * (end - a.0);
                }
                area
            }
        };
        integral * duration_s
    }

    /// Returns the displacement of a whole glide.
    pub fn total(&self, duration_s: f64) -> f64 {
        self.displacement_at(1.0, duration_s)
    }

    fn knots(
        &self,
        points: &[(f64, f64)],
        segment: impl Fn((f64, f64), (f64, f64)) -> Option<f64>,
    ) -> f64 {
        let Some(first) = points.first() else {
            return 0.0;
        };
        if points.len() == 1 {
            return first.1;
        }
        points
            .windows(2)
            .find_map(|w| segment(w[0], w[1]))
            .unwrap_or_else(|| points.last().map_or(0.0, |p| p.1))
    }
}

fn finite(v: f64) -> Result<(), MotionError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(MotionError::NotFinite("velocity"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(curve: &VelocityCurve, f: f64, d: f64) -> f64 {
        let n = 100_000;
        (0..n)
            .map(|i| curve.velocity_at((i as f64 + 0.5) / n as f64 * f) * f / n as f64)
            .sum::<f64>()
            * d
    }

    #[test]
    fn integrals_match_numeric_integration() {
        let curves = [
            VelocityCurve::constant(2400.0),
            VelocityCurve::ramp(0.0, 3000.0),
            VelocityCurve::keyframes([(0.0, 0.0), (0.2, 3000.0), (0.8, 3000.0), (1.0, 0.0)]),
            VelocityCurve::keyframes([(0.3, 500.0), (0.6, -500.0)]),
            VelocityCurve::keyframes([(0.5, 800.0)]),
        ];
        for c in &curves {
            for f in [0.0, 0.1, 0.25, 0.5, 0.77, 1.0] {
                let exact = c.displacement_at(f, 2.0);
                let approx = numeric(c, f, 2.0);
                assert!(
                    (exact - approx).abs() < 1e-3,
                    "{c:?} at {f}: {exact} vs {approx}"
                );
            }
        }
        assert_eq!(VelocityCurve::constant(2400.0).total(2.0), 4800.0);
        assert_eq!(VelocityCurve::ramp(0.0, 3000.0).total(1.0), 1500.0);
    }

    #[test]
    fn keyframe_validation() {
        assert!(VelocityCurve::keyframes([]).validate().is_err());
        assert!(
            VelocityCurve::keyframes([(0.5, 1.0), (0.2, 1.0)])
                .validate()
                .is_err()
        );
        assert!(VelocityCurve::keyframes([(1.5, 1.0)]).validate().is_err());
        assert!(VelocityCurve::constant(f64::NAN).validate().is_err());
        assert!(VelocityCurve::keyframes([(0.0, 1.0)]).validate().is_ok());
    }

    #[test]
    fn json_form() {
        let c: VelocityCurve =
            serde_json::from_str(r#"{"type":"keyframes","points":[[0,0],[0.5,3000],[1,0]]}"#)
                .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            c,
            VelocityCurve::keyframes([(0.0, 0.0), (0.5, 3000.0), (1.0, 0.0)])
        );
    }
}
