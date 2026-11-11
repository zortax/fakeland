//! The geometry of a viewer window: the initial window size, the letterboxed place of the output
//! picture inside the window, and the map from window coordinates to output coordinates.

/// A rectangle in window coordinates of the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// The largest initial window, in host logical pixels.
pub const MAX_INITIAL: (i32, i32) = (1280, 800);

/// Returns the initial window size of an output: its logical size, scaled down to fit
/// `MAX_INITIAL`.
pub fn initial_size(output: (i32, i32)) -> (i32, i32) {
    let (w, h) = (output.0.max(1) as f64, output.1.max(1) as f64);
    let s = (f64::from(MAX_INITIAL.0) / w)
        .min(f64::from(MAX_INITIAL.1) / h)
        .min(1.0);
    ((w * s).round() as i32, (h * s).round() as i32)
}

/// Fits a picture of size `picture` into a window of size `window`, keeping the aspect ratio
/// and centered.
pub fn fit(picture: (i32, i32), window: (i32, i32)) -> ContentRect {
    let (pw, ph) = (picture.0.max(1) as f64, picture.1.max(1) as f64);
    let (ww, wh) = (window.0.max(1), window.1.max(1));
    let scale = (f64::from(ww) / pw).min(f64::from(wh) / ph);
    let w = ((pw * scale).round() as i32).clamp(1, ww);
    let h = ((ph * scale).round() as i32).clamp(1, wh);
    ContentRect {
        x: (ww - w) / 2,
        y: (wh - h) / 2,
        w,
        h,
    }
}

/// Maps a point in window coordinates to output logical coordinates, clamped to the output.
pub fn to_output(content: ContentRect, output: (i32, i32), x: f64, y: f64) -> (f64, f64) {
    let fx = ((x - f64::from(content.x)) / f64::from(content.w.max(1))).clamp(0.0, 1.0);
    let fy = ((y - f64::from(content.y)) / f64::from(content.h.max(1))).clamp(0.0, 1.0);
    let max_x = (f64::from(output.0) - 1e-3).max(0.0);
    let max_y = (f64::from(output.1) - 1e-3).max(0.0);
    (
        (fx * f64::from(output.0)).min(max_x),
        (fy * f64::from(output.1)).min(max_y),
    )
}

/// Returns the factor from output pixels to window pixels, used to bound CPU readbacks.
pub fn shrink_factor(physical: (i32, i32), shown: (u32, u32)) -> f64 {
    let fx = f64::from(shown.0) / f64::from(physical.0.max(1));
    let fy = f64::from(shown.1) / f64::from(physical.1.max(1));
    fx.max(fy).clamp(0.05, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_size_keeps_aspect_and_never_grows() {
        assert_eq!(initial_size((1920, 1080)), (1280, 720));
        assert_eq!(initial_size((800, 600)), (800, 600));
        assert_eq!(initial_size((1280, 1024)), (1000, 800));
    }

    #[test]
    fn letterboxes_into_a_wide_window() {
        let r = fit((1920, 1080), (1000, 1000));
        assert_eq!(
            r,
            ContentRect {
                x: 0,
                y: 218,
                w: 1000,
                h: 563
            }
        );
        let r = fit((1920, 1080), (3840, 2160));
        assert_eq!((r.w, r.h), (3840, 2160));
    }

    #[test]
    fn maps_window_points_to_the_output() {
        let r = fit((1920, 1080), (960, 600));
        assert_eq!(
            r,
            ContentRect {
                x: 0,
                y: 30,
                w: 960,
                h: 540
            }
        );
        assert_eq!(to_output(r, (1920, 1080), 480.0, 300.0), (960.0, 540.0));
        let (x, y) = to_output(r, (1920, 1080), 2000.0, 0.0);
        assert!(x < 1920.0 && x > 1919.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn readback_shrinks_to_the_shown_size() {
        assert_eq!(shrink_factor((1920, 1080), (960, 540)), 0.5);
        assert_eq!(shrink_factor((1920, 1080), (4000, 2000)), 1.0);
    }
}
