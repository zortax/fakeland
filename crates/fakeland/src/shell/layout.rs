//! Window placement: the geometry of a new window under the window policy, and the geometry of
//! each placement state on an output.

use fakeland_types::{Logical, Rect, Size};

use super::Placement;
use crate::config::WindowPolicy;

/// The cascade step between floating windows.
const CASCADE_STEP: i32 = 32;

/// A placement with its geometry in the global logical space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The placement state.
    pub placement: Placement,
    /// The window geometry.
    pub rect: Rect<Logical>,
}

/// Returns the layout of the `index`-th new window on an output.
pub fn initial_layout(policy: WindowPolicy, output: Rect<Logical>, index: usize) -> Layout {
    match policy {
        WindowPolicy::Kiosk => Layout {
            placement: Placement::Maximized,
            rect: output,
        },
        WindowPolicy::Fullscreen => Layout {
            placement: Placement::Fullscreen,
            rect: output,
        },
        WindowPolicy::Floating { default_size } => Layout {
            placement: Placement::Floating,
            rect: cascade(output, default_size, index),
        },
    }
}

/// Places a floating window of `size` centered on the output, shifted down and right by the
/// cascade step per index, and kept inside the output where it fits.
pub fn cascade(output: Rect<Logical>, size: Size<Logical>, index: usize) -> Rect<Logical> {
    let w = size.w.min(output.w).max(1);
    let h = size.h.min(output.h).max(1);
    let step = CASCADE_STEP * (index % 8) as i32;
    let x = output.x + ((output.w - w) / 2 + step).min(output.w - w).max(0);
    let y = output.y + ((output.h - h) / 2 + step).min(output.h - h).max(0);
    Rect::new(x, y, w, h)
}

/// Returns the geometry of a placement on an output. A floating window keeps `floating`.
pub fn placement_rect(
    placement: Placement,
    output: Rect<Logical>,
    floating: Rect<Logical>,
) -> Rect<Logical> {
    match placement {
        Placement::Maximized | Placement::Fullscreen => output,
        Placement::Floating => floating,
    }
}

/// Returns the origin of a window of `actual` size. A maximized or fullscreen window that stays
/// smaller than its output sits centered on it.
pub fn window_origin(
    placement: Placement,
    output: Rect<Logical>,
    floating: Rect<Logical>,
    actual: Size<Logical>,
) -> (i32, i32) {
    match placement {
        Placement::Floating => (floating.x, floating.y),
        Placement::Maximized | Placement::Fullscreen => {
            let center = |origin: i32, room: i32, size: i32| {
                if size > 0 && size < room {
                    origin + (room - size) / 2
                } else {
                    origin
                }
            };
            (
                center(output.x, output.w, actual.w),
                center(output.y, output.h, actual.h),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kiosk_and_fullscreen_fill_the_output() {
        let out = Rect::new(1920, 0, 1280, 720);
        let l = initial_layout(WindowPolicy::Kiosk, out, 3);
        assert_eq!(l.placement, Placement::Maximized);
        assert_eq!(l.rect, out);
        assert_eq!(
            initial_layout(WindowPolicy::Fullscreen, out, 0).placement,
            Placement::Fullscreen
        );
    }

    #[test]
    fn small_maximized_windows_sit_centered() {
        let out = Rect::new(100, 0, 1000, 800);
        let floating = Rect::new(5, 6, 10, 10);
        assert_eq!(
            window_origin(Placement::Maximized, out, floating, Size::new(400, 300)),
            (400, 250)
        );
        assert_eq!(
            window_origin(Placement::Fullscreen, out, floating, Size::new(1000, 800)),
            (100, 0)
        );
        assert_eq!(
            window_origin(Placement::Maximized, out, floating, Size::new(0, 0)),
            (100, 0)
        );
        assert_eq!(
            window_origin(Placement::Floating, out, floating, Size::new(400, 300)),
            (5, 6)
        );
    }

    #[test]
    fn floating_windows_cascade_inside_the_output() {
        let out = Rect::new(0, 0, 1000, 800);
        let size = Size::new(400, 300);
        let policy = WindowPolicy::Floating { default_size: size };
        assert_eq!(
            initial_layout(policy, out, 0).rect,
            Rect::new(300, 250, 400, 300)
        );
        assert_eq!(
            initial_layout(policy, out, 1).rect,
            Rect::new(332, 282, 400, 300)
        );
        let big = cascade(out, Size::new(2000, 2000), 5);
        assert_eq!(big, Rect::new(0, 0, 1000, 800));
    }
}
