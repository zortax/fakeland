//! Software drawing into XRGB8888 frames: colors, a scaled canvas, a bitmap digit font, glyph
//! boxes and frame differences.

mod canvas;
mod color;
mod diff;
mod font;
mod glyph_box;

pub use canvas::{BYTES_PER_PIXEL, Canvas};
pub use color::Color;
pub use diff::{changed_bbox, changed_rows};
pub use font::{draw_text, text_height, text_width};
pub use glyph_box::{GlyphBoxStyle, draw_glyph_box};
