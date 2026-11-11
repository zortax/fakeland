//! The type scene: it echoes typed characters as glyph boxes in rows from the top-left corner.
//! Key presses with text append it, Return starts a new row, BackSpace removes the last
//! character, and text-input commit strings append their text.

use crate::draw::{Canvas, GlyphBoxStyle, draw_glyph_box};
use crate::input::InputEffect;

/// The xkb keysym of Return.
const KEY_RETURN: u32 = 0xff0d;
/// The xkb keysym of keypad Enter.
const KEY_KP_ENTER: u32 = 0xff8d;
/// The xkb keysym of BackSpace.
const KEY_BACKSPACE: u32 = 0xff08;
/// Logical margin around the text.
const MARGIN: f64 = 16.0;
/// Logical gap between boxes.
const GAP: f64 = 4.0;

/// The typed text.
#[derive(Clone, Debug, Default)]
pub struct TypeScene {
    text: Vec<char>,
    style: GlyphBoxStyle,
}

impl TypeScene {
    /// Builds an empty text.
    pub fn new(style: GlyphBoxStyle) -> Self {
        Self {
            text: Vec::new(),
            style,
        }
    }

    /// Returns the typed text.
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// Applies an input effect. Returns true when the text changed.
    pub fn apply(&mut self, effect: &InputEffect) -> bool {
        match effect {
            InputEffect::Key {
                keysym,
                utf8,
                pressed: true,
                ..
            } => match *keysym {
                KEY_RETURN | KEY_KP_ENTER => self.push('\n'),
                KEY_BACKSPACE => self.text.pop().is_some(),
                _ => self.push_str(utf8),
            },
            InputEffect::Text { text } => self.push_str(text),
            _ => false,
        }
    }

    /// Draws the glyph boxes.
    pub fn draw(&self, canvas: &mut Canvas<'_>) {
        let width = canvas.logical_width();
        let (step_x, step_y) = (self.style.width + GAP, self.style.height + GAP);
        let (mut x, mut y) = (MARGIN, MARGIN);
        for &c in &self.text {
            if c == '\n' {
                x = MARGIN;
                y += step_y;
                continue;
            }
            if x + self.style.width > width - MARGIN && x > MARGIN {
                x = MARGIN;
                y += step_y;
            }
            draw_glyph_box(canvas, x, y, c, &self.style);
            x += step_x;
        }
    }

    fn push_str(&mut self, text: &str) -> bool {
        let mut changed = false;
        for c in text.chars() {
            changed |= self.push(c);
        }
        changed
    }

    fn push(&mut self, c: char) -> bool {
        if c != '\n' && c.is_control() {
            return false;
        }
        self.text.push(c);
        true
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Size;

    use super::*;
    use crate::draw::Color;

    fn key(keysym: u32, utf8: &str) -> InputEffect {
        InputEffect::Key {
            code: 30,
            keysym,
            utf8: utf8.to_owned(),
            pressed: true,
        }
    }

    #[test]
    fn edits_the_text() {
        let mut s = TypeScene::default();
        assert!(s.apply(&key(0x61, "a")));
        assert!(s.apply(&key(0x62, "b")));
        assert!(s.apply(&key(KEY_RETURN, "\r")));
        assert!(s.apply(&InputEffect::Text {
            text: "€x".to_owned()
        }));
        assert_eq!(s.text(), "ab\n€x");
        assert!(s.apply(&key(KEY_BACKSPACE, "\u{8}")));
        assert_eq!(s.text(), "ab\n€");
        // Shift alone produces no text.
        assert!(!s.apply(&key(0xffe1, "")));
        // Releases change nothing.
        let release = InputEffect::Key {
            code: 30,
            keysym: 0x61,
            utf8: "a".to_owned(),
            pressed: false,
        };
        assert!(!s.apply(&release));
        // Control characters are ignored.
        assert!(!s.apply(&key(0xff09, "\t")));
    }

    #[test]
    fn backspace_on_empty_text_changes_nothing() {
        let mut s = TypeScene::default();
        assert!(!s.apply(&key(KEY_BACKSPACE, "\u{8}")));
    }

    #[test]
    fn draws_one_box_per_character() {
        let mut s = TypeScene::default();
        s.apply(&key(0x61, "a"));
        s.apply(&key(0x62, "b"));
        let mut data = vec![0; 200 * 100 * 4];
        let mut c = Canvas::new(&mut data, Size::new(200, 100), 1.0);
        s.draw(&mut c);
        let ink = GlyphBoxStyle::default().ink;
        // The outline corners of both boxes.
        assert_eq!(c.pixel(16, 16), Some(ink));
        assert_eq!(c.pixel(16 + 28, 16), Some(ink));
        assert_eq!(c.pixel(16 + 56, 16), Some(Color::BLACK));
    }
}
