//! The hover scene: a target rectangle that shows its active color while the pointer is inside.

use fakeland_types::{Logical, Rect};

use crate::draw::{Canvas, Color};
use crate::input::InputEffect;

/// A hover target and its state.
#[derive(Clone, Debug)]
pub struct HoverScene {
    rect: Rect<Logical>,
    idle: Color,
    active: Color,
    inside: bool,
}

impl HoverScene {
    /// Builds a hover target.
    pub fn new(rect: Rect<Logical>, idle: Color, active: Color) -> Self {
        Self {
            rect,
            idle,
            active,
            inside: false,
        }
    }

    /// Returns true while the pointer is inside the target.
    pub fn inside(&self) -> bool {
        self.inside
    }

    /// Applies an input effect. Returns true when the visible state changed.
    pub fn apply(&mut self, effect: &InputEffect) -> bool {
        let inside = match effect {
            InputEffect::PointerEnter { pos } | InputEffect::PointerMotion { pos } => {
                self.rect.contains(*pos)
            }
            InputEffect::PointerLeave => false,
            _ => return false,
        };
        let changed = inside != self.inside;
        self.inside = inside;
        changed
    }

    /// Draws the target.
    pub fn draw(&self, canvas: &mut Canvas<'_>) {
        let color = if self.inside { self.active } else { self.idle };
        canvas.fill_rect(self.rect, color);
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Point, Size};

    use super::*;

    fn scene() -> HoverScene {
        HoverScene::new(Rect::new(10, 10, 20, 20), Color::BLACK, Color::WHITE)
    }

    #[test]
    fn enters_and_leaves() {
        let mut s = scene();
        assert!(!s.apply(&InputEffect::PointerEnter {
            pos: Point::new(0.0, 0.0)
        }));
        assert!(s.apply(&InputEffect::PointerMotion {
            pos: Point::new(10.0, 10.0)
        }));
        assert!(s.inside());
        assert!(!s.apply(&InputEffect::PointerMotion {
            pos: Point::new(29.9, 29.9)
        }));
        assert!(s.apply(&InputEffect::PointerMotion {
            pos: Point::new(30.0, 15.0)
        }));
        assert!(!s.inside());
        s.apply(&InputEffect::PointerMotion {
            pos: Point::new(15.0, 15.0),
        });
        assert!(s.apply(&InputEffect::PointerLeave));
        assert!(!s.apply(&InputEffect::TouchUp { id: 0 }));
    }

    #[test]
    fn draws_the_state_color() {
        let mut s = scene();
        let mut data = vec![0; 40 * 40 * 4];
        let mut c = Canvas::new(&mut data, Size::new(40, 40), 1.0);
        c.clear(Color::rgb(1, 1, 1));
        s.draw(&mut c);
        assert_eq!(c.pixel(15, 15), Some(Color::BLACK));
        s.apply(&InputEffect::PointerMotion {
            pos: Point::new(15.0, 15.0),
        });
        s.draw(&mut c);
        assert_eq!(c.pixel(15, 15), Some(Color::WHITE));
        assert_eq!(c.pixel(5, 5), Some(Color::rgb(1, 1, 1)));
    }
}
