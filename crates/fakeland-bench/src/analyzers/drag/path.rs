//! The injected pointer path of a drag.

use fakeland_trace::{FrameTimeline, InputKind};
use fakeland_types::Timestamp;

/// The pointer positions of a drag in send order, in the global logical space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerPath {
    points: Vec<(Timestamp, f64, f64)>,
}

impl PointerPath {
    /// Builds a path from timed positions in send order.
    pub fn new(points: Vec<(Timestamp, f64, f64)>) -> Self {
        Self { points }
    }

    /// Returns the motions while a button is held, or every motion when no button event exists.
    pub fn of_drag(tl: &FrameTimeline) -> Self {
        let inputs = tl.inputs();
        let press = inputs
            .iter()
            .find(|i| i.kind == InputKind::PointerButton && i.detail.contains("pressed"))
            .map(|i| i.sent);
        let release = inputs
            .iter()
            .filter(|i| press.is_some_and(|p| i.sent >= p))
            .find(|i| i.kind == InputKind::PointerButton && i.detail.contains("released"))
            .map(|i| i.sent);
        let points = inputs
            .iter()
            .filter(|i| i.kind == InputKind::PointerMotion)
            .filter(|i| press.is_none_or(|p| i.sent >= p) && release.is_none_or(|r| i.sent <= r))
            .filter_map(|i| Some((i.sent, i.x?, i.y?)))
            .collect();
        Self { points }
    }

    /// Returns the count of positions.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true without positions.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns the first and last send time.
    pub fn span(&self) -> Option<(Timestamp, Timestamp)> {
        Some((self.points.first()?.0, self.points.last()?.0))
    }

    /// Returns the unit vector from the first to the last position and the distance.
    pub fn direction(&self) -> Option<((f64, f64), f64)> {
        let (_, x0, y0) = *self.points.first()?;
        let (_, x1, y1) = *self.points.last()?;
        let d = (x1 - x0).hypot(y1 - y0);
        (d > f64::EPSILON).then(|| (((x1 - x0) / d, (y1 - y0) / d), d))
    }

    /// Returns the displacement along `dir` of the last position sent at or before `t`.
    pub fn along_at(&self, t: Timestamp, dir: (f64, f64)) -> f64 {
        let Some(&(_, x0, y0)) = self.points.first() else {
            return 0.0;
        };
        self.points
            .iter()
            .take_while(|p| p.0 <= t)
            .last()
            .map_or(0.0, |&(_, x, y)| (x - x0) * dir.0 + (y - y0) * dir.1)
    }

    /// Returns the first time the displacement along `dir` reached `target`, interpolated between
    /// positions. Returns `None` when the path never reaches it.
    pub fn time_reaching(&self, target: f64, dir: (f64, f64)) -> Option<Timestamp> {
        let &(t0, x0, y0) = self.points.first()?;
        if target <= 0.0 {
            return Some(t0);
        }
        let along = |&(_, x, y): &(Timestamp, f64, f64)| (x - x0) * dir.0 + (y - y0) * dir.1;
        self.points.windows(2).find_map(|w| {
            let (a, b) = (along(&w[0]), along(&w[1]));
            if b < target || b <= a {
                return None;
            }
            let f = ((target - a) / (b - a)).clamp(0.0, 1.0);
            let dt = w[1].0.since(w[0].0).as_nanos() as f64 * f;
            Some(w[0].0 + fakeland_types::Nanos(dt.round() as i64))
        })
    }
}
