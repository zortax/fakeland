//! Evaluation of a validated path. A sampler holds the frame and the precomputed geometry so each
//! sample is a cheap interpolation.

use std::f64::consts::TAU;

use super::{MotionError, Path};
use crate::Position;

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Polyline {
        points: Vec<(f64, f64)>,
        cumulative: Vec<f64>,
    },
    Bezier([(f64, f64); 4]),
    Circle {
        center: (f64, f64),
        radius: f64,
        turns: f64,
    },
}

/// A path that is ready for sampling.
#[derive(Clone, Debug, PartialEq)]
pub struct PathSampler {
    frame: Position,
    shape: Shape,
}

impl PathSampler {
    pub(super) fn new(path: &Path) -> Result<Self, MotionError> {
        let positions = path.positions();
        let Some(frame) = positions.first().map(|p| (*p).clone()) else {
            return Err(MotionError::EmptyPath);
        };
        if !positions.iter().all(|p| p.same_frame(&frame)) {
            return Err(MotionError::MixedFrames);
        }
        if !positions.iter().all(|p| p.is_finite()) {
            return Err(MotionError::NotFinite("path coordinate"));
        }
        let shape = match path {
            Path::Line { from, to } => polyline(vec![from.coords(), to.coords()]),
            Path::Polyline { points } => polyline(points.iter().map(Position::coords).collect()),
            Path::Bezier { p0, p1, p2, p3 } => {
                Shape::Bezier([p0.coords(), p1.coords(), p2.coords(), p3.coords()])
            }
            Path::Circle {
                center,
                radius,
                turns,
            } => {
                if !radius.is_finite() || !turns.is_finite() {
                    return Err(MotionError::NotFinite("circle radius or turns"));
                }
                Shape::Circle {
                    center: center.coords(),
                    radius: *radius,
                    turns: *turns,
                }
            }
        };
        Ok(Self { frame, shape })
    }

    /// Returns the position at progress `t`, clamped to `0..=1`.
    pub fn point_at(&self, t: f64) -> Position {
        let (x, y) = self.coords_at(t.clamp(0.0, 1.0));
        self.frame.with_coords(x, y)
    }

    /// Returns the start position.
    pub fn start(&self) -> Position {
        self.point_at(0.0)
    }

    /// Returns the end position.
    pub fn end(&self) -> Position {
        self.point_at(1.0)
    }

    /// Returns the path length in the units of its frame.
    pub fn length(&self) -> f64 {
        match &self.shape {
            Shape::Polyline { cumulative, .. } => cumulative.last().copied().unwrap_or(0.0),
            Shape::Bezier(_) => {
                let steps = 256;
                (1..=steps)
                    .map(|i| {
                        let a = self.coords_at((i - 1) as f64 / steps as f64);
                        let b = self.coords_at(i as f64 / steps as f64);
                        ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
                    })
                    .sum()
            }
            Shape::Circle { radius, turns, .. } => (TAU * radius * turns).abs(),
        }
    }

    fn coords_at(&self, t: f64) -> (f64, f64) {
        match &self.shape {
            Shape::Polyline { points, cumulative } => along_polyline(points, cumulative, t),
            Shape::Bezier([p0, p1, p2, p3]) => {
                let u = 1.0 - t;
                let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                (
                    w[0] * p0.0 + w[1] * p1.0 + w[2] * p2.0 + w[3] * p3.0,
                    w[0] * p0.1 + w[1] * p1.1 + w[2] * p2.1 + w[3] * p3.1,
                )
            }
            Shape::Circle {
                center,
                radius,
                turns,
            } => {
                let angle = TAU * turns * t;
                (
                    center.0 + radius * angle.cos(),
                    center.1 + radius * angle.sin(),
                )
            }
        }
    }
}

fn polyline(points: Vec<(f64, f64)>) -> Shape {
    let mut cumulative = Vec::with_capacity(points.len());
    let mut total = 0.0;
    cumulative.push(0.0);
    for w in points.windows(2) {
        total += ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        cumulative.push(total);
    }
    Shape::Polyline { points, cumulative }
}

fn along_polyline(points: &[(f64, f64)], cumulative: &[f64], t: f64) -> (f64, f64) {
    let total = cumulative.last().copied().unwrap_or(0.0);
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return (0.0, 0.0);
    };
    if total <= 0.0 {
        return if t >= 1.0 { *last } else { *first };
    }
    let target = t * total;
    let seg = cumulative
        .windows(2)
        .position(|w| target <= w[1])
        .unwrap_or(points.len().saturating_sub(2));
    let (a, b) = (points[seg], points[seg + 1]);
    let len = cumulative[seg + 1] - cumulative[seg];
    let f = if len > 0.0 {
        (target - cumulative[seg]) / len
    } else {
        1.0
    };
    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowRef;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn line_endpoints_and_midpoint() {
        let p = Path::line(Position::output(0.0, 0.0), Position::output(100.0, 50.0));
        let s = p.sampler().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.start(), Position::output(0.0, 0.0));
        assert_eq!(s.end(), Position::output(100.0, 50.0));
        assert!(close(s.point_at(0.5).coords(), (50.0, 25.0)));
        assert!((s.length() - 111.803_398_874_989_5).abs() < 1e-9);
    }

    #[test]
    fn polyline_moves_at_constant_speed() {
        let p = Path::polyline([
            Position::output(0.0, 0.0),
            Position::output(100.0, 0.0),
            Position::output(100.0, 300.0),
        ]);
        let s = p.sampler().unwrap_or_else(|e| panic!("{e}"));
        assert!(close(s.point_at(0.25).coords(), (100.0, 0.0)));
        assert!(close(s.point_at(0.5).coords(), (100.0, 100.0)));
        assert!(close(s.end().coords(), (100.0, 300.0)));
        let single = Path::polyline([Position::output(5.0, 5.0)]);
        assert!(close(
            single
                .point_at(0.7)
                .unwrap_or_else(|e| panic!("{e}"))
                .coords(),
            (5.0, 5.0)
        ));
        assert_eq!(Path::polyline([]).sampler(), Err(MotionError::EmptyPath));
    }

    #[test]
    fn bezier_hits_its_ends() {
        let w = WindowRef::app_id("a");
        let p = Path::bezier(
            Position::window(w.clone(), 0.0, 0.0),
            Position::window(w.clone(), 0.0, 100.0),
            Position::window(w.clone(), 100.0, 100.0),
            Position::window(w.clone(), 100.0, 0.0),
        );
        let s = p.sampler().unwrap_or_else(|e| panic!("{e}"));
        assert!(close(s.start().coords(), (0.0, 0.0)));
        assert!(close(s.end().coords(), (100.0, 0.0)));
        assert!(close(s.point_at(0.5).coords(), (50.0, 75.0)));
        assert_eq!(s.point_at(0.5), Position::window(w, 50.0, 75.0));
    }

    #[test]
    fn circle_returns_to_its_start() {
        let p = Path::circle(Position::output(100.0, 100.0), 10.0, 1.0);
        let s = p.sampler().unwrap_or_else(|e| panic!("{e}"));
        assert!(close(s.start().coords(), (110.0, 100.0)));
        assert!(close(s.point_at(0.25).coords(), (100.0, 110.0)));
        let end = s.end().coords();
        assert!((end.0 - 110.0).abs() < 1e-9 && (end.1 - 100.0).abs() < 1e-9);
        assert!((s.length() - TAU * 10.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_frames_fail() {
        let p = Path::line(Position::output(0.0, 0.0), Position::center(WindowRef::Any));
        assert_eq!(p.sampler(), Err(MotionError::MixedFrames));
        let mut p = p;
        p.map_positions(|q| Position::output(q.coords().0, q.coords().1));
        assert!(p.sampler().is_ok());
    }
}
