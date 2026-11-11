//! Pointer paths: lines, polylines, cubic Bézier curves and circles over symbolic positions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{MotionError, PathSampler};
use crate::Position;

/// A path through positions in one coordinate frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Path {
    /// A straight line.
    Line {
        /// The start.
        from: Position,
        /// The end.
        to: Position,
    },
    /// Straight segments through points, at constant speed along the whole length.
    Polyline {
        /// The points in order.
        points: Vec<Position>,
    },
    /// A cubic Bézier curve.
    Bezier {
        /// The start.
        p0: Position,
        /// The first control point.
        p1: Position,
        /// The second control point.
        p2: Position,
        /// The end.
        p3: Position,
    },
    /// A circle that starts east of the center and turns clockwise on screen.
    Circle {
        /// The center.
        center: Position,
        /// The radius in the units of the center frame.
        radius: f64,
        /// The number of turns. Negative values turn counterclockwise.
        turns: f64,
    },
}

impl Path {
    /// Builds a line.
    pub fn line(from: Position, to: Position) -> Self {
        Self::Line { from, to }
    }

    /// Builds a polyline.
    pub fn polyline(points: impl IntoIterator<Item = Position>) -> Self {
        Self::Polyline {
            points: points.into_iter().collect(),
        }
    }

    /// Builds a cubic Bézier curve.
    pub fn bezier(p0: Position, p1: Position, p2: Position, p3: Position) -> Self {
        Self::Bezier { p0, p1, p2, p3 }
    }

    /// Builds a circle.
    pub fn circle(center: Position, radius: f64, turns: f64) -> Self {
        Self::Circle {
            center,
            radius,
            turns,
        }
    }

    /// Returns every position that the path names.
    pub fn positions(&self) -> Vec<&Position> {
        match self {
            Self::Line { from, to } => vec![from, to],
            Self::Polyline { points } => points.iter().collect(),
            Self::Bezier { p0, p1, p2, p3 } => vec![p0, p1, p2, p3],
            Self::Circle { center, .. } => vec![center],
        }
    }

    /// Replaces every position, for example to resolve mixed frames into output coordinates.
    pub fn map_positions(&mut self, mut f: impl FnMut(&Position) -> Position) {
        match self {
            Self::Line { from, to } => {
                *from = f(from);
                *to = f(to);
            }
            Self::Polyline { points } => points.iter_mut().for_each(|p| *p = f(p)),
            Self::Bezier { p0, p1, p2, p3 } => {
                for p in [p0, p1, p2, p3] {
                    *p = f(p);
                }
            }
            Self::Circle { center, .. } => *center = f(center),
        }
    }

    /// Prepares the path for evaluation. Fails when the points use different frames.
    pub fn sampler(&self) -> Result<PathSampler, MotionError> {
        PathSampler::new(self)
    }

    /// Returns the position at progress `t` in `0..=1`.
    pub fn point_at(&self, t: f64) -> Result<Position, MotionError> {
        Ok(self.sampler()?.point_at(t))
    }
}
