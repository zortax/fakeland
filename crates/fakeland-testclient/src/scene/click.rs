//! The click scene: a rectangle that toggles between two colors on each button press or touch
//! down inside it, with the press count drawn in its corner. Without a configured rectangle the
//! scene covers the whole window.

use fakeland_types::{Logical, Point, Rect, Size};

use crate::draw::{Canvas, Color, draw_text};
use crate::input::InputEffect;

/// Logical size of one font pixel of the press counter.
const COUNTER_CELL: f64 = 3.0;

/// A click target and its state.
#[derive(Clone, Debug)]
pub struct ClickScene {
    rect: Option<Rect<Logical>>,
    colors: [Color; 2],
    ink: Color,
    toggled: bool,
    presses: u64,
}

impl ClickScene {
    /// Builds a click target. `None` covers the whole window.
    pub fn new(rect: Option<Rect<Logical>>, off: Color, on: Color) -> Self {
        Self {
            rect,
            colors: [off, on],
            ink: Color::WHITE,
            toggled: false,
            presses: 0,
        }
    }

    /// Returns true when an odd number of presses landed in the target.
    pub fn toggled(&self) -> bool {
        self.toggled
    }

    /// Returns the number of presses that landed in the target.
    pub fn presses(&self) -> u64 {
        self.presses
    }

    /// Applies an input effect in a window of a logical size. Returns true when the visible
    /// state changed.
    pub fn apply(&mut self, effect: &InputEffect, window: Size<Logical>) -> bool {
        let pos = match effect {
            InputEffect::Button {
                pressed: true,
                pos: Some(pos),
                ..
            } => *pos,
            InputEffect::TouchDown { pos, .. } => *pos,
            _ => return false,
        };
        if !self.hit(pos, window) {
            return false;
        }
        self.toggled = !self.toggled;
        self.presses += 1;
        true
    }

    /// Draws the target.
    pub fn draw(&self, canvas: &mut Canvas<'_>, window: Size<Logical>) {
        let rect = self.area(window);
        canvas.fill_rect(rect, self.colors[self.toggled as usize]);
        draw_text(
            canvas,
            rect.x as f64 + 8.0,
            rect.y as f64 + 8.0,
            COUNTER_CELL,
            &self.presses.to_string(),
            self.ink,
        );
    }

    fn area(&self, window: Size<Logical>) -> Rect<Logical> {
        self.rect.unwrap_or(Rect::from_size(window))
    }

    fn hit(&self, pos: Point<Logical>, window: Size<Logical>) -> bool {
        self.area(window).contains(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(x: f64, y: f64) -> InputEffect {
        InputEffect::Button {
            button: 0x110,
            pressed: true,
            pos: Some(Point::new(x, y)),
        }
    }

    #[test]
    fn toggles_on_press_inside() {
        let window = Size::new(100, 100);
        let mut s = ClickScene::new(Some(Rect::new(0, 0, 50, 50)), Color::BLACK, Color::WHITE);
        assert!(s.apply(&press(10.0, 10.0), window));
        assert!(s.toggled());
        assert!(!s.apply(&press(60.0, 10.0), window));
        let release = InputEffect::Button {
            button: 0x110,
            pressed: false,
            pos: Some(Point::new(10.0, 10.0)),
        };
        assert!(!s.apply(&release, window));
        assert!(s.apply(
            &InputEffect::TouchDown {
                id: 0,
                pos: Point::new(1.0, 1.0)
            },
            window
        ));
        assert!(!s.toggled());
        assert_eq!(s.presses(), 2);
    }

    #[test]
    fn whole_window_by_default() {
        let window = Size::new(100, 100);
        let mut s = ClickScene::new(None, Color::BLACK, Color::WHITE);
        assert!(s.apply(&press(99.0, 99.0), window));
        let mut data = vec![0; 100 * 100 * 4];
        let mut c = Canvas::new(&mut data, Size::new(100, 100), 1.0);
        s.draw(&mut c, window);
        assert_eq!(c.pixel(90, 90), Some(Color::WHITE));
    }
}
