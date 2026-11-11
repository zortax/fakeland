//! A software canvas over a packed XRGB8888 pixel buffer. Callers draw in logical coordinates;
//! the canvas scales them to buffer pixels and rounds each edge to the nearest pixel, so shapes
//! that share an edge tile without gaps or overlap.

use fakeland_types::{Logical, Physical, Rect, Size};

use super::Color;

/// Bytes per XRGB8888 pixel.
pub const BYTES_PER_PIXEL: usize = 4;

/// A drawing target: packed rows of XRGB8888 pixels and a logical-to-buffer scale.
#[derive(Debug)]
pub struct Canvas<'a> {
    data: &'a mut [u8],
    size: Size<Physical>,
    scale: f64,
}

impl<'a> Canvas<'a> {
    /// Builds a canvas over `data`, which holds `size.w * size.h` packed pixels.
    ///
    /// # Panics
    /// Panics when `data` is shorter than the pixel area or the scale is not positive.
    pub fn new(data: &'a mut [u8], size: Size<Physical>, scale: f64) -> Self {
        let needed = size.w.max(0) as usize * size.h.max(0) as usize * BYTES_PER_PIXEL;
        assert!(data.len() >= needed, "canvas buffer too small");
        assert!(
            scale > 0.0 && scale.is_finite(),
            "canvas scale must be positive"
        );
        Self { data, size, scale }
    }

    /// Returns the buffer size in pixels.
    pub fn size(&self) -> Size<Physical> {
        self.size
    }

    /// Returns the buffer pixels per logical unit.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Returns the bytes per row.
    pub fn stride(&self) -> usize {
        self.size.w.max(0) as usize * BYTES_PER_PIXEL
    }

    /// Returns the logical width of the canvas.
    pub fn logical_width(&self) -> f64 {
        self.size.w as f64 / self.scale
    }

    /// Returns the logical height of the canvas.
    pub fn logical_height(&self) -> f64 {
        self.size.h as f64 / self.scale
    }

    /// Converts a logical coordinate to the nearest buffer pixel edge. Halves round up, so a
    /// shape moved by a whole number of pixels keeps its pixel size.
    pub fn to_px(&self, v: f64) -> i64 {
        (v * self.scale + 0.5).floor() as i64
    }

    /// Fills the whole canvas.
    pub fn clear(&mut self, color: Color) {
        self.fill_px_edges(0, 0, self.size.w as i64, self.size.h as i64, color);
    }

    /// Fills a logical rectangle.
    pub fn fill_rect(&mut self, rect: Rect<Logical>, color: Color) {
        self.fill_edges(
            rect.x as f64,
            rect.y as f64,
            (rect.x + rect.w) as f64,
            (rect.y + rect.h) as f64,
            color,
        );
    }

    /// Fills the logical area between the edges `x0..x1` and `y0..y1`.
    pub fn fill_edges(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color) {
        let (px0, py0, px1, py1) = (
            self.to_px(x0),
            self.to_px(y0),
            self.to_px(x1),
            self.to_px(y1),
        );
        self.fill_px_edges(px0, py0, px1, py1, color);
    }

    /// Draws a logical rectangle outline of the given logical thickness, inside the rectangle.
    pub fn stroke_rect(&mut self, rect: Rect<Logical>, thickness: f64, color: Color) {
        let (x0, y0) = (rect.x as f64, rect.y as f64);
        let (x1, y1) = ((rect.x + rect.w) as f64, (rect.y + rect.h) as f64);
        let t = thickness.min(rect.w as f64 / 2.0).min(rect.h as f64 / 2.0);
        self.fill_edges(x0, y0, x1, y0 + t, color);
        self.fill_edges(x0, y1 - t, x1, y1, color);
        self.fill_edges(x0, y0 + t, x0 + t, y1 - t, color);
        self.fill_edges(x1 - t, y0 + t, x1, y1 - t, color);
    }

    /// Returns the pixel at a buffer position, or `None` outside the canvas.
    #[cfg(test)]
    pub fn pixel(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.size.w || y >= self.size.h {
            return None;
        }
        let i = y as usize * self.stride() + x as usize * BYTES_PER_PIXEL;
        let p = &self.data[i..i + BYTES_PER_PIXEL];
        Some(Color::from_pixel([p[0], p[1], p[2], p[3]]))
    }

    /// Fills buffer pixels `x0..x1` times `y0..y1`, clipped. The first row segment fills by
    /// doubling copies and every further row copies that segment, so a fill costs a few
    /// memcpy calls per row.
    fn fill_px_edges(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: Color) {
        let x0 = x0.clamp(0, self.size.w as i64) as usize;
        let x1 = x1.clamp(0, self.size.w as i64) as usize;
        let y0 = y0.clamp(0, self.size.h as i64) as usize;
        let y1 = y1.clamp(0, self.size.h as i64) as usize;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let stride = self.stride();
        let first = y0 * stride + x0 * BYTES_PER_PIXEL;
        let len = (x1 - x0) * BYTES_PER_PIXEL;
        let segment = &mut self.data[first..first + len];
        segment[..BYTES_PER_PIXEL].copy_from_slice(&color.pixel());
        let mut filled = BYTES_PER_PIXEL;
        while filled < len {
            let n = filled.min(len - filled);
            segment.copy_within(..n, filled);
            filled += n;
        }
        for y in y0 + 1..y1 {
            let start = y * stride + x0 * BYTES_PER_PIXEL;
            self.data.copy_within(first..first + len, start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(w: i32, h: i32) -> Vec<u8> {
        vec![0; w as usize * h as usize * BYTES_PER_PIXEL]
    }

    #[test]
    fn clear_and_fill() {
        let mut data = buf(4, 3);
        let mut c = Canvas::new(&mut data, Size::new(4, 3), 1.0);
        c.clear(Color::WHITE);
        c.fill_rect(Rect::new(1, 1, 2, 1), Color::BLACK);
        assert_eq!(c.pixel(0, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(1, 1), Some(Color::BLACK));
        assert_eq!(c.pixel(2, 1), Some(Color::BLACK));
        assert_eq!(c.pixel(3, 1), Some(Color::WHITE));
        assert_eq!(c.pixel(4, 1), None);
    }

    #[test]
    fn fill_clips() {
        let mut data = buf(4, 4);
        let mut c = Canvas::new(&mut data, Size::new(4, 4), 1.0);
        c.fill_rect(Rect::new(-10, -10, 12, 12), Color::WHITE);
        assert_eq!(c.pixel(1, 1), Some(Color::WHITE));
        assert_eq!(c.pixel(2, 2), Some(Color::BLACK));
        c.fill_rect(Rect::new(10, 10, 5, 5), Color::WHITE);
    }

    #[test]
    fn scaled_edges_round_and_tile() {
        let mut data = buf(15, 1);
        let mut c = Canvas::new(&mut data, Size::new(15, 1), 1.5);
        // Logical 0..5 is pixels 0..8 (7.5 rounds to 8); 5..10 is pixels 8..15.
        c.fill_edges(0.0, 0.0, 5.0, 1.0, Color::WHITE);
        c.fill_edges(5.0, 0.0, 10.0, 1.0, Color::rgb(9, 9, 9));
        assert_eq!(c.pixel(7, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(8, 0), Some(Color::rgb(9, 9, 9)));
        assert_eq!(c.pixel(14, 0), Some(Color::rgb(9, 9, 9)));
        assert!((c.logical_width() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn stroke_leaves_inside() {
        let mut data = buf(6, 6);
        let mut c = Canvas::new(&mut data, Size::new(6, 6), 1.0);
        c.stroke_rect(Rect::new(0, 0, 6, 6), 1.0, Color::WHITE);
        assert_eq!(c.pixel(0, 3), Some(Color::WHITE));
        assert_eq!(c.pixel(5, 3), Some(Color::WHITE));
        assert_eq!(c.pixel(3, 0), Some(Color::WHITE));
        assert_eq!(c.pixel(3, 5), Some(Color::WHITE));
        assert_eq!(c.pixel(2, 2), Some(Color::BLACK));
    }
}
