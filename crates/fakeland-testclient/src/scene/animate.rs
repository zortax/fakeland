//! The animate scene: a vertical bar that moves right at a constant logical speed and wraps at
//! the window edge. Its position is a pure function of the animation time, so the position on
//! each presented frame shows the pacing of the frames.

use fakeland_types::Timestamp;

use crate::draw::{Canvas, Color};

/// A moving bar.
#[derive(Clone, Debug)]
pub struct AnimateScene {
    speed: f64,
    width: f64,
    color: Color,
    start: Option<Timestamp>,
}

impl AnimateScene {
    /// Builds a bar that moves `speed` logical pixels per second and is `width` wide.
    pub fn new(speed: f64, width: f64, color: Color) -> Self {
        Self {
            speed,
            width,
            color,
            start: None,
        }
    }

    /// Returns the unwrapped travelled distance at a time, in logical pixels.
    pub fn distance(&self, t: Timestamp) -> f64 {
        match self.start {
            Some(start) if t > start => t.since(start).as_secs_f64() * self.speed,
            _ => 0.0,
        }
    }

    /// Returns the left edge of the bar at a time in a window of a logical width.
    pub fn position(&self, t: Timestamp, window_width: f64) -> f64 {
        if window_width <= 0.0 {
            return 0.0;
        }
        self.distance(t).rem_euclid(window_width)
    }

    /// Draws the bar for a time and returns its left edge. The first call fixes the start.
    pub fn draw(&mut self, canvas: &mut Canvas<'_>, t: Timestamp) -> f64 {
        self.start.get_or_insert(t);
        let (w, h) = (canvas.logical_width(), canvas.logical_height());
        let x = self.position(t, w);
        canvas.fill_edges(x, 0.0, (x + self.width).min(w), h, self.color);
        if x + self.width > w {
            canvas.fill_edges(0.0, 0.0, x + self.width - w, h, self.color);
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Nanos, Size};

    use super::*;

    #[test]
    fn moves_at_constant_speed() {
        let mut s = AnimateScene::new(600.0, 40.0, Color::WHITE);
        let mut data = vec![0; 1000 * 10 * 4];
        let mut c = Canvas::new(&mut data, Size::new(1000, 10), 1.0);
        let t0 = Timestamp(1_000_000_000);
        assert_eq!(s.draw(&mut c, t0), 0.0);
        let t1 = t0 + Nanos::from_millis(100);
        assert!((s.position(t1, 1000.0) - 60.0).abs() < 1e-9);
        let t2 = t0 + Nanos::from_secs(2);
        // 1200 px wraps to 200 in a 1000 px window.
        assert!((s.position(t2, 1000.0) - 200.0).abs() < 1e-9);
        assert!((s.distance(t2) - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn bar_wraps_at_the_edge() {
        let mut s = AnimateScene::new(100.0, 40.0, Color::WHITE);
        let mut data = vec![0; 100 * 2 * 4];
        let mut c = Canvas::new(&mut data, Size::new(100, 2), 1.0);
        let t0 = Timestamp(0);
        s.draw(&mut c, t0);
        let x = s.draw(&mut c, t0 + Nanos::from_millis(800));
        assert!((x - 80.0).abs() < 1e-9);
        assert_eq!(c.pixel(85, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(10, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(25, 0), Some(Color::WHITE));
    }
}
