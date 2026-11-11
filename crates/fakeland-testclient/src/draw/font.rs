//! A tiny built-in 5x7 bitmap font for digits and the few symbols that labels need: `x`, `-`,
//! `.`, `:` and space. Each font pixel is a square of `cell` logical units.

use super::{Canvas, Color};

/// Font pixel columns per glyph.
pub const GLYPH_COLUMNS: u32 = 5;
/// Font pixel rows per glyph.
pub const GLYPH_ROWS: u32 = 7;
/// Font pixel columns from one glyph to the next.
pub const ADVANCE_COLUMNS: u32 = GLYPH_COLUMNS + 1;

/// Returns the rows of a glyph, top to bottom. Bit 4 is the leftmost column.
pub fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        'x' => [0x00, 0x00, 0x11, 0x0a, 0x04, 0x0a, 0x11],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        ':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        ' ' => [0; 7],
        _ => return None,
    })
}

/// Returns the logical width of a text at a cell size. The last glyph has no trailing gap.
pub fn text_width(text: &str, cell: f64) -> f64 {
    let n = text.chars().count() as f64;
    if n == 0.0 {
        return 0.0;
    }
    (n * ADVANCE_COLUMNS as f64 - 1.0) * cell
}

/// Returns the logical height of a text line at a cell size.
pub fn text_height(cell: f64) -> f64 {
    GLYPH_ROWS as f64 * cell
}

/// Draws a text with its top-left corner at a logical position. Characters without a glyph
/// leave a blank cell.
pub fn draw_text(canvas: &mut Canvas<'_>, x: f64, y: f64, cell: f64, text: &str, color: Color) {
    for (i, c) in text.chars().enumerate() {
        let Some(rows) = glyph(c) else { continue };
        let gx = x + (i as u32 * ADVANCE_COLUMNS) as f64 * cell;
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..GLYPH_COLUMNS {
                if bits & (1 << (GLYPH_COLUMNS - 1 - col)) != 0 {
                    let px = gx + col as f64 * cell;
                    let py = y + row as f64 * cell;
                    canvas.fill_edges(px, py, px + cell, py + cell, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Size;

    use super::*;

    #[test]
    fn every_digit_has_a_distinct_glyph() {
        let glyphs: Vec<_> = ('0'..='9').map(|c| glyph(c).unwrap()).collect();
        for (i, a) in glyphs.iter().enumerate() {
            assert!(a.iter().all(|r| *r < 32));
            for b in &glyphs[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert!(glyph('?').is_none());
    }

    #[test]
    fn width_counts_gaps() {
        assert_eq!(text_width("", 2.0), 0.0);
        assert_eq!(text_width("1", 2.0), 10.0);
        assert_eq!(text_width("12", 2.0), 22.0);
        assert_eq!(text_height(3.0), 21.0);
    }

    #[test]
    fn draws_one_glyph() {
        let mut data = vec![0; 6 * 7 * 4];
        let mut c = Canvas::new(&mut data, Size::new(6, 7), 1.0);
        draw_text(&mut c, 0.0, 0.0, 1.0, "1", Color::WHITE);
        // Row 0 of '1' is 00100: only column 2 is set.
        assert_eq!(c.pixel(2, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(1, 0), Some(Color::BLACK));
        // Row 6 is 01110.
        for x in 1..4 {
            assert_eq!(c.pixel(x, 6), Some(Color::WHITE));
        }
        assert_eq!(c.pixel(0, 6), Some(Color::BLACK));
        assert_eq!(c.pixel(5, 6), Some(Color::BLACK));
    }
}
