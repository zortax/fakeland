//! Glyph boxes: one outlined box per character whose inner 4x4 grid shows the low 16 bits of the
//! code point. Bit 0 is the top-left cell, and the bits run row by row. Every character in the
//! Basic Multilingual Plane gets a distinct pattern, so a screenshot shows which text arrived.

use super::{Canvas, Color};

/// Cells per side of the bit grid.
pub const GRID: u32 = 4;

/// The logical layout of a glyph box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphBoxStyle {
    /// Box width.
    pub width: f64,
    /// Box height.
    pub height: f64,
    /// Outline thickness.
    pub border: f64,
    /// Outline and set-bit color.
    pub ink: Color,
    /// Fill color of the box.
    pub paper: Color,
}

impl Default for GlyphBoxStyle {
    fn default() -> Self {
        Self {
            width: 24.0,
            height: 32.0,
            border: 2.0,
            ink: Color::rgb(0xf0, 0xf0, 0xf0),
            paper: Color::rgb(0x30, 0x30, 0x60),
        }
    }
}

/// Returns true when bit `index` of the grid is set for a character.
pub fn bit_set(c: char, index: u32) -> bool {
    (c as u32 & 0xffff) & (1 << index) != 0
}

/// Draws the glyph box of `c` with its top-left corner at a logical position.
pub fn draw_glyph_box(canvas: &mut Canvas<'_>, x: f64, y: f64, c: char, style: &GlyphBoxStyle) {
    let (w, h, b) = (style.width, style.height, style.border);
    canvas.fill_edges(x, y, x + w, y + h, style.paper);
    canvas.fill_edges(x, y, x + w, y + b, style.ink);
    canvas.fill_edges(x, y + h - b, x + w, y + h, style.ink);
    canvas.fill_edges(x, y + b, x + b, y + h - b, style.ink);
    canvas.fill_edges(x + w - b, y + b, x + w, y + h - b, style.ink);
    let inner = Inner::of(x, y, style);
    for index in 0..GRID * GRID {
        if bit_set(c, index) {
            let (cx0, cy0, cx1, cy1) = inner.cell(index);
            canvas.fill_edges(cx0, cy0, cx1, cy1, style.ink);
        }
    }
}

/// The bit grid area inside the outline, with one border width of padding.
struct Inner {
    x: f64,
    y: f64,
    cw: f64,
    ch: f64,
}

impl Inner {
    fn of(x: f64, y: f64, style: &GlyphBoxStyle) -> Self {
        let pad = style.border * 2.0;
        let cw = (style.width - 2.0 * pad).max(0.0) / GRID as f64;
        let ch = (style.height - 2.0 * pad).max(0.0) / GRID as f64;
        Self {
            x: x + pad,
            y: y + pad,
            cw,
            ch,
        }
    }

    fn cell(&self, index: u32) -> (f64, f64, f64, f64) {
        let (col, row) = ((index % GRID) as f64, (index / GRID) as f64);
        let x0 = self.x + col * self.cw;
        let y0 = self.y + row * self.ch;
        (x0, y0, x0 + self.cw, y0 + self.ch)
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Logical, Rect, Size};

    use super::*;

    fn cell_rect(x: f64, y: f64, index: u32, style: &GlyphBoxStyle) -> Rect<Logical> {
        let (x0, y0, x1, y1) = Inner::of(x, y, style).cell(index);
        Rect::new(
            x0.floor() as i32,
            y0.floor() as i32,
            (x1.ceil() - x0.floor()) as i32,
            (y1.ceil() - y0.floor()) as i32,
        )
    }

    #[test]
    fn bits_follow_the_code_point() {
        // 'A' is 0x41: bits 0 and 6.
        assert!(bit_set('A', 0));
        assert!(bit_set('A', 6));
        assert!(!bit_set('A', 1));
        assert!(!bit_set('A', 15));
    }

    #[test]
    fn draws_the_bit_pattern() {
        let style = GlyphBoxStyle::default();
        let mut data = vec![0; 24 * 32 * 4];
        let mut canvas = Canvas::new(&mut data, Size::new(24, 32), 1.0);
        draw_glyph_box(&mut canvas, 0.0, 0.0, 'A', &style);
        let center = |i: u32| {
            let r = cell_rect(0.0, 0.0, i, &style);
            (r.x + r.w / 2, r.y + r.h / 2)
        };
        for i in 0..16 {
            let (x, y) = center(i);
            let want = if bit_set('A', i) {
                style.ink
            } else {
                style.paper
            };
            assert_eq!(canvas.pixel(x, y), Some(want), "cell {i}");
        }
        assert_eq!(canvas.pixel(0, 0), Some(style.ink));
    }

    #[test]
    fn distinct_characters_differ() {
        let style = GlyphBoxStyle::default();
        let render = |c: char| {
            let mut data = vec![0; 24 * 32 * 4];
            let mut canvas = Canvas::new(&mut data, Size::new(24, 32), 1.0);
            draw_glyph_box(&mut canvas, 0.0, 0.0, c, &style);
            data
        };
        assert_ne!(render('a'), render('b'));
        assert_eq!(render('a'), render('a'));
    }
}
