//! The fixed picture size of a recording and the fit of each captured image into it. A video
//! needs one size for its whole length and even dimensions for 4:2:0 chroma. An image one pixel
//! larger than the canvas loses its last row or column; an image of another size scales to fit
//! and centers on black.

use fakeland_types::{Physical, Rect};
use fakeland_vision::{Image, ImageView};

/// The pixel size of a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    /// The width, even and at least 2.
    pub width: u32,
    /// The height, even and at least 2.
    pub height: u32,
}

impl Canvas {
    /// Returns the canvas of a capture of `width` by `height` pixels: the size rounds down to
    /// even dimensions.
    pub fn of(width: u32, height: u32) -> Self {
        let even = |v: u32| (v - v % 2).max(2);
        Self {
            width: even(width.max(1)),
            height: even(height.max(1)),
        }
    }
}

/// Returns the factor that brings the longer side of `width` by `height` to at most `max_dim`.
pub fn fit_factor(width: f64, height: f64, max_dim: Option<u32>) -> f64 {
    let longest = width.max(height);
    match max_dim {
        Some(max) if max > 0 && longest > f64::from(max) => f64::from(max) / longest,
        _ => 1.0,
    }
}

/// A frame placed on the canvas.
pub enum Fitted<'a> {
    /// A view of the captured image with the canvas size.
    View(ImageView<'a>),
    /// A scaled copy with the canvas size.
    Scaled(Image),
}

impl Fitted<'_> {
    /// Returns a view with the canvas size.
    pub fn view(&self) -> ImageView<'_> {
        match self {
            Self::View(v) => *v,
            Self::Scaled(i) => i.view(),
        }
    }
}

/// Places an image on the canvas.
pub fn fit<'a>(image: &'a Image, canvas: Canvas) -> Fitted<'a> {
    let (w, h) = (image.width(), image.height());
    let close = |v: u32, c: u32| v == c || v == c + 1;
    if close(w, canvas.width) && close(h, canvas.height) {
        let rect = Rect::<Physical>::new(0, 0, canvas.width as i32, canvas.height as i32);
        if let Some(view) = image.view().region(rect) {
            return Fitted::View(view);
        }
    }
    Fitted::Scaled(letterbox(image, canvas))
}

/// Scales an image to fit the canvas, keeps its aspect ratio and centers it on black. Each
/// output pixel is the mean of the source pixels it covers, or the nearest source pixel when the
/// image grows.
pub fn letterbox(image: &Image, canvas: Canvas) -> Image {
    let (sw, sh) = (image.width().max(1) as f64, image.height().max(1) as f64);
    let (cw, ch) = (f64::from(canvas.width), f64::from(canvas.height));
    let scale = (cw / sw).min(ch / sh);
    let dw = ((sw * scale).round() as u32).clamp(1, canvas.width);
    let dh = ((sh * scale).round() as u32).clamp(1, canvas.height);
    let (ox, oy) = ((canvas.width - dw) / 2, (canvas.height - dh) / 2);
    let mut out = Image::filled(canvas.width, canvas.height, [0, 0, 0, 255]);
    let (fx, fy) = (sw / f64::from(dw), sh / f64::from(dh));
    let src = image.view();
    for y in 0..dh {
        let y0 = (f64::from(y) * fy) as u32;
        let y1 = ((f64::from(y + 1) * fy).ceil() as u32).clamp(y0 + 1, image.height());
        for x in 0..dw {
            let x0 = (f64::from(x) * fx) as u32;
            let x1 = ((f64::from(x + 1) * fx).ceil() as u32).clamp(x0 + 1, image.width());
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                let row = src.row(sy);
                for sx in x0..x1 {
                    let i = sx as usize * 4;
                    for (a, c) in acc.iter_mut().zip(&row[i..i + 4]) {
                        *a += u32::from(*c);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let px = acc.map(|a| ((a + n / 2) / n) as u8);
            out.set_pixel(ox + x, oy + y, [px[0], px[1], px[2], 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_rounds_down_to_even() {
        assert_eq!(
            Canvas::of(1281, 721),
            Canvas {
                width: 1280,
                height: 720
            }
        );
        assert_eq!(
            Canvas::of(1, 0),
            Canvas {
                width: 2,
                height: 2
            }
        );
    }

    #[test]
    fn fit_factor_limits_the_longer_side() {
        assert_eq!(fit_factor(1920.0, 1080.0, Some(960)), 0.5);
        assert_eq!(fit_factor(1920.0, 1080.0, Some(4000)), 1.0);
        assert_eq!(fit_factor(1920.0, 1080.0, None), 1.0);
    }

    #[test]
    fn one_extra_pixel_crops_without_a_copy() {
        let img = Image::filled(641, 361, [9, 8, 7, 255]);
        let canvas = Canvas::of(641, 361);
        match fit(&img, canvas) {
            Fitted::View(v) => assert_eq!((v.width(), v.height()), (640, 360)),
            Fitted::Scaled(_) => panic!("expected a view"),
        }
    }

    #[test]
    fn other_sizes_letterbox_on_black() {
        let img = Image::filled(100, 100, [200, 100, 50, 255]);
        let canvas = Canvas::of(200, 100);
        let Fitted::Scaled(out) = fit(&img, canvas) else {
            panic!("expected a scaled copy");
        };
        assert_eq!((out.width(), out.height()), (200, 100));
        assert_eq!(out.pixel(0, 50), Some([0, 0, 0, 255]));
        assert_eq!(out.pixel(100, 50), Some([200, 100, 50, 255]));
        assert_eq!(out.pixel(199, 50), Some([0, 0, 0, 255]));
    }

    #[test]
    fn downscale_averages_and_upscale_repeats() {
        let mut img = Image::filled(4, 2, [0, 0, 0, 255]);
        img.set_pixel(0, 0, [255, 255, 255, 255]);
        img.set_pixel(1, 0, [255, 255, 255, 255]);
        let half = letterbox(&img, Canvas::of(2, 1));
        assert_eq!((half.width(), half.height()), (2, 2));
        let up = letterbox(&Image::filled(1, 1, [10, 20, 30, 255]), Canvas::of(4, 4));
        assert_eq!(up.pixel(3, 3), Some([10, 20, 30, 255]));
    }
}
