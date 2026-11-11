//! The resize scene: a border at the window edge, the logical size as large digits in the center
//! and the buffer size and scale below it. A screenshot shows which configure the frame answers.

use fakeland_types::{Logical, Rect, Size};

use crate::draw::{Canvas, Color, draw_text, text_height, text_width};

/// Logical border thickness.
const BORDER: f64 = 4.0;

/// The size display.
#[derive(Clone, Debug)]
pub struct ResizeScene {
    ink: Color,
}

impl ResizeScene {
    /// Builds a size display.
    pub fn new(ink: Color) -> Self {
        Self { ink }
    }

    /// Returns the two text lines for a logical window size and the canvas.
    pub fn lines(window: Size<Logical>, canvas: &Canvas<'_>) -> (String, String) {
        let buffer = canvas.size();
        let scale = format!("{:.3}", canvas.scale());
        let scale = scale.trim_end_matches('0').trim_end_matches('.');
        (
            format!("{}x{}", window.w, window.h),
            format!("{}x{} {}", buffer.w, buffer.h, scale),
        )
    }

    /// Draws the border and the size lines.
    pub fn draw(&self, canvas: &mut Canvas<'_>, window: Size<Logical>) {
        canvas.stroke_rect(Rect::from_size(window), BORDER, self.ink);
        let (big, small) = Self::lines(window, canvas);
        let (w, h) = (window.w as f64, window.h as f64);
        let fit = |text: &str, max_cell: f64| {
            let unit = text_width(text, 1.0).max(1.0);
            (((w - 4.0 * BORDER) / unit).floor()).clamp(1.0, max_cell)
        };
        let big_cell = fit(&big, 8.0);
        let small_cell = fit(&small, 3.0);
        let total = text_height(big_cell) + 3.0 * small_cell + text_height(small_cell);
        let top = ((h - total) / 2.0).max(BORDER);
        let bx = ((w - text_width(&big, big_cell)) / 2.0).max(BORDER);
        draw_text(canvas, bx.round(), top.round(), big_cell, &big, self.ink);
        let sy = top + text_height(big_cell) + 3.0 * small_cell;
        let sx = ((w - text_width(&small, small_cell)) / 2.0).max(BORDER);
        draw_text(canvas, sx.round(), sy.round(), small_cell, &small, self.ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_show_logical_and_buffer_size() {
        let mut data = vec![0; 150 * 90 * 4];
        let c = Canvas::new(&mut data, Size::new(150, 90), 1.5);
        let (big, small) = ResizeScene::lines(Size::new(100, 60), &c);
        assert_eq!(big, "100x60");
        assert_eq!(small, "150x90 1.5");
        let mut data = vec![0; 4 * 4 * 4];
        let c = Canvas::new(&mut data, Size::new(4, 4), 2.0);
        assert_eq!(ResizeScene::lines(Size::new(2, 2), &c).1, "4x4 2");
    }

    #[test]
    fn draws_border_and_text() {
        let mut data = vec![0; 300 * 200 * 4];
        let mut c = Canvas::new(&mut data, Size::new(300, 200), 1.0);
        ResizeScene::new(Color::WHITE).draw(&mut c, Size::new(300, 200));
        assert_eq!(c.pixel(0, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(299, 199), Some(Color::WHITE));
        assert_eq!(c.pixel(10, 10), Some(Color::BLACK));
        let lit = (0..200)
            .flat_map(|y| (0..300).map(move |x| (x, y)))
            .filter(|&(x, y)| (8..292).contains(&x) && (8..192).contains(&y))
            .filter(|&(x, y)| c.pixel(x, y) == Some(Color::WHITE))
            .count();
        assert!(lit > 100);
    }
}
