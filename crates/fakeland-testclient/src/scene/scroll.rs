//! The scroll scene: an endless list of numbered stripes. The content offset is exactly the sum
//! of the scroll amounts times a factor, without clamping, inertia or smoothing, so the
//! displacement on screen equals the input displacement.

use serde::Serialize;

use crate::draw::{Canvas, Color, draw_text, text_height};
use crate::input::{AxisFrame, AxisSource, InputEffect};

/// Logical size of one font pixel of the stripe labels.
const LABEL_CELL: f64 = 3.0;
/// Logical distance between the vertical tick marks.
const TICK_SPACING: f64 = 80.0;
/// Logical width of a tick mark.
const TICK_WIDTH: f64 = 4.0;
/// Stripe colors, cycled by stripe index.
const PALETTE: [Color; 4] = [
    Color::rgb(0x2a, 0x4d, 0x69),
    Color::rgb(0x4b, 0x86, 0xb4),
    Color::rgb(0x63, 0x9a, 0x67),
    Color::rgb(0x8f, 0x5b, 0x8a),
];

/// How the scene turns a scroll frame into a content offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ScrollUnit {
    /// Use the wl_pointer.axis value in logical pixels.
    Pixels,
    /// Use wheel steps for wheel frames (value120, or discrete steps on old compositors) times
    /// the wheel step, and pixels for every other source.
    Value120,
}

/// The scroll list and its offset.
#[derive(Clone, Debug)]
pub struct ScrollScene {
    unit: ScrollUnit,
    wheel_step: f64,
    factor: f64,
    stripe: f64,
    offset_x: f64,
    offset_y: f64,
}

impl ScrollScene {
    /// Builds a scroll list.
    pub fn new(unit: ScrollUnit, wheel_step: f64, factor: f64, stripe: f64) -> Self {
        Self {
            unit,
            wheel_step,
            factor,
            stripe,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// Returns the content offset in logical pixels. Positive values show content further right
    /// and further down.
    pub fn offset(&self) -> (f64, f64) {
        (self.offset_x, self.offset_y)
    }

    /// Returns the scroll amount of a frame in logical pixels, before the factor.
    pub fn amount(&self, axis: &AxisFrame) -> (f64, f64) {
        let wheel = matches!(axis.source, Some(AxisSource::Wheel | AxisSource::WheelTilt));
        if self.unit == ScrollUnit::Value120 && wheel {
            let step = |v120: i32, discrete: i32, px: f64| {
                if v120 != 0 {
                    v120 as f64 / 120.0 * self.wheel_step
                } else if discrete != 0 {
                    discrete as f64 * self.wheel_step
                } else {
                    px
                }
            };
            return (
                step(axis.value120_x, axis.discrete_x, axis.dx),
                step(axis.value120_y, axis.discrete_y, axis.dy),
            );
        }
        (axis.dx, axis.dy)
    }

    /// Applies an input effect. Returns true when the offset changed.
    pub fn apply(&mut self, effect: &InputEffect) -> bool {
        let InputEffect::Axis(axis) = effect else {
            return false;
        };
        let (dx, dy) = self.amount(axis);
        self.offset_x += dx * self.factor;
        self.offset_y += dy * self.factor;
        dx != 0.0 || dy != 0.0
    }

    /// Draws the visible stripes.
    pub fn draw(&self, canvas: &mut Canvas<'_>) {
        let (w, h) = (canvas.logical_width(), canvas.logical_height());
        let first = (self.offset_y / self.stripe).floor() as i64;
        let mut index = first;
        loop {
            let y0 = index as f64 * self.stripe - self.offset_y;
            if y0 >= h {
                break;
            }
            let y1 = y0 + self.stripe;
            let color = PALETTE[index.rem_euclid(PALETTE.len() as i64) as usize];
            canvas.fill_edges(0.0, y0, w, y1, color);
            self.draw_ticks(canvas, y0, y1, w);
            let label_y = y0 + (self.stripe - text_height(LABEL_CELL)) / 2.0;
            let stagger = index.rem_euclid(7) as f64 * 37.0;
            let label_x = 12.0 + stagger - self.offset_x;
            draw_text(
                canvas,
                label_x,
                label_y,
                LABEL_CELL,
                &index.to_string(),
                Color::WHITE,
            );
            index += 1;
        }
    }

    fn draw_ticks(&self, canvas: &mut Canvas<'_>, y0: f64, y1: f64, w: f64) {
        let ink = Color::rgb(0x10, 0x10, 0x10);
        let mut k = (self.offset_x / TICK_SPACING).floor() as i64;
        loop {
            let x = k as f64 * TICK_SPACING - self.offset_x;
            if x >= w {
                break;
            }
            let tall = k.rem_euclid(5) == 0;
            let top = if tall { y0 } else { y0 + (y1 - y0) * 0.6 };
            canvas.fill_edges(x, top, x + TICK_WIDTH, y1, ink);
            k += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Size;

    use super::*;

    fn wheel(v120: i32, px: f64) -> InputEffect {
        InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Wheel),
            dy: px,
            value120_y: v120,
            ..AxisFrame::default()
        })
    }

    #[test]
    fn pixels_unit_sums_axis_values() {
        let mut s = ScrollScene::new(ScrollUnit::Pixels, 15.0, 1.0, 40.0);
        assert!(s.apply(&wheel(120, 15.0)));
        assert!(s.apply(&wheel(60, 7.5)));
        assert_eq!(s.offset(), (0.0, 22.5));
        let finger = InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Finger),
            dx: 3.25,
            dy: -1.5,
            ..AxisFrame::default()
        });
        assert!(s.apply(&finger));
        assert_eq!(s.offset(), (3.25, 21.0));
    }

    #[test]
    fn value120_unit_uses_wheel_steps() {
        let mut s = ScrollScene::new(ScrollUnit::Value120, 20.0, 2.0, 40.0);
        s.apply(&wheel(60, 7.5));
        assert_eq!(s.offset(), (0.0, 20.0));
        let old = InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Wheel),
            dy: 10.0,
            discrete_y: -1,
            ..AxisFrame::default()
        });
        s.apply(&old);
        assert_eq!(s.offset(), (0.0, -20.0));
        let finger = InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Finger),
            dy: 5.0,
            ..AxisFrame::default()
        });
        s.apply(&finger);
        assert_eq!(s.offset(), (0.0, -10.0));
    }

    #[test]
    fn stop_frames_change_nothing() {
        let mut s = ScrollScene::new(ScrollUnit::Pixels, 15.0, 1.0, 40.0);
        let stop = InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Finger),
            stop_y: true,
            ..AxisFrame::default()
        });
        assert!(!s.apply(&stop));
    }

    #[test]
    fn content_moves_by_the_offset() {
        let render = |s: &ScrollScene| {
            let mut data = vec![0; 200 * 200 * 4];
            let mut c = Canvas::new(&mut data, Size::new(200, 200), 1.0);
            s.draw(&mut c);
            data
        };
        let mut s = ScrollScene::new(ScrollUnit::Pixels, 15.0, 1.0, 40.0);
        let before = render(&s);
        s.apply(&wheel(0, 30.0));
        let after = render(&s);
        // Row 30 + y of the old frame is row y of the new frame.
        let row = |f: &[u8], y: usize| f[y * 800..(y + 1) * 800].to_vec();
        for y in 0..170 {
            assert_eq!(row(&after, y), row(&before, y + 30), "row {y}");
        }
        assert_ne!(before, after);
    }

    #[test]
    fn negative_offsets_draw() {
        let mut s = ScrollScene::new(ScrollUnit::Pixels, 15.0, 1.0, 40.0);
        s.apply(&wheel(0, -1000.0));
        let mut data = vec![0; 100 * 100 * 4];
        let mut c = Canvas::new(&mut data, Size::new(100, 100), 1.0);
        s.draw(&mut c);
        assert!(c.pixel(50, 50).is_some_and(|p| p != Color::BLACK));
    }
}
