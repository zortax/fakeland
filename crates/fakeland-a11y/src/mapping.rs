//! Coordinate mapping from app-reported extents to the global logical space.
//!
//! On Wayland an app does not know where its window is, so toolkits report extents relative to
//! their toplevel window, even for screen coordinates. The origin of that space differs between
//! toolkits: some use the window geometry, some use the surface with its client-side shadows,
//! and some report physical pixels. The frame node of a window has the extents of the whole
//! window, so its size selects the origin and the unit: the frame size matches either the window
//! geometry or the surface bounding box, at scale 1 or at the output scale. A frame that is a
//! little smaller than the geometry is the client area inside client-side decorations, as Qt
//! draws them: equal borders left, right and bottom, and a title bar on top. A frame that matches
//! none of these leaves the reported coordinates relative to the window geometry.
//!
//! Some toolkits mix units inside one window: Chromium reports its own controls in logical units
//! and web content in physical pixels. A subtree switches to physical pixels when a node overflows
//! its parent horizontally in logical units and fits it after division by the output scale (see
//! [`Mapping::descend`]). The rule reads widths and positions only, so content taller than its
//! viewport still switches. Content that is wider than its viewport and in logical units stays
//! logical. GTK 4 leaves CSS margins out of its extents, so its positions can be off by those
//! margins while its sizes are right.

use fakeland_types::{Logical, Rect, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CoordSpace, Extents, WindowFrame};

/// What the frame extents matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// The frame covers the window geometry.
    Geometry,
    /// The frame covers the surface with its shadows.
    Surface,
    /// The frame covers the client area inside a title bar and borders.
    ClientArea,
    /// The frame matches neither, and the coordinates count from the window geometry.
    Assumed,
}

/// The map from reported extents of one window to global logical coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mapping {
    /// The window.
    pub window: WindowId,
    /// The global logical x of the reported origin.
    pub origin_x: f64,
    /// The global logical y of the reported origin.
    pub origin_y: f64,
    /// Logical units per reported unit.
    pub factor: f64,
    /// The output scale of the window.
    pub scale: f64,
    /// What the frame extents matched.
    pub anchor: Anchor,
    /// The coordinate space that the app answers in.
    pub space: CoordSpace,
}

impl Mapping {
    /// Derives the mapping of a window from the extents of its frame node.
    pub fn calibrate(frame: Option<Extents>, window: &WindowFrame, space: CoordSpace) -> Self {
        let assumed = Self {
            window: window.id,
            origin_x: window.geometry.x as f64,
            origin_y: window.geometry.y as f64,
            factor: 1.0,
            scale: window.scale,
            anchor: Anchor::Assumed,
            space,
        };
        let Some(f) = frame.filter(|f| f.w > 0 && f.h > 0) else {
            return assumed;
        };
        for factor in Self::factors(window.scale) {
            for (anchor, rect) in [
                (Anchor::Geometry, window.geometry),
                (Anchor::Surface, window.surface),
            ] {
                if close(f.w as f64 * factor, rect.w) && close(f.h as f64 * factor, rect.h) {
                    return Self {
                        window: window.id,
                        origin_x: rect.x as f64 - f.x as f64 * factor,
                        origin_y: rect.y as f64 - f.y as f64 * factor,
                        factor,
                        scale: window.scale,
                        anchor,
                        space,
                    };
                }
            }
        }
        for factor in Self::factors(window.scale) {
            let (fw, fh) = (f.w as f64 * factor, f.h as f64 * factor);
            let g = window.geometry;
            let side = (g.w as f64 - fw) / 2.0;
            let top = g.h as f64 - fh - side;
            if (-1.0..=MAX_BORDER).contains(&side) && (-1.0..=MAX_TITLE_BAR).contains(&top) {
                return Self {
                    window: window.id,
                    origin_x: g.x as f64 + side.max(0.0) - f.x as f64 * factor,
                    origin_y: g.y as f64 + top.max(0.0) - f.y as f64 * factor,
                    factor,
                    scale: window.scale,
                    anchor: Anchor::ClientArea,
                    space,
                };
            }
        }
        assumed
    }

    /// Returns the unit factors to try: logical first, then physical pixels.
    fn factors(scale: f64) -> Vec<f64> {
        let mut factors = vec![1.0];
        if scale.is_finite() && scale > 0.0 && (scale - 1.0).abs() > 1e-6 {
            factors.push(1.0 / scale);
        }
        factors
    }

    /// Returns the mapping of a node below a parent. It switches to physical pixels when the node
    /// overflows the parent horizontally and fits it after division by the scale.
    pub fn descend(&self, parent: Option<Extents>, node: Option<Extents>) -> Self {
        let (Some(p), Some(n)) = (parent, node) else {
            return *self;
        };
        let physical = self.factor / self.scale;
        let switchable = self.scale.is_finite()
            && self.scale > 1.0 + 1e-6
            && (self.factor - physical).abs() > 1e-9
            && (self.factor - 1.0).abs() < 1e-9;
        if !switchable || p.w <= 0 || n.w <= 0 {
            return *self;
        }
        let tol = 2.0;
        let (px0, px1, py0) = (p.x as f64, (p.x + p.w) as f64, p.y as f64);
        let overflows = (n.x + n.w) as f64 > px1 + tol || (n.x as f64) < px0 - tol;
        let s = self.scale;
        let (sx0, sx1, sy0) = (n.x as f64 / s, (n.x + n.w) as f64 / s, n.y as f64 / s);
        let fits = sx0 >= px0 - tol && sx1 <= px1 + tol && sy0 >= py0 - tol;
        if overflows && fits {
            Self {
                factor: physical,
                ..*self
            }
        } else {
            *self
        }
    }

    /// Maps reported extents to a global logical rectangle.
    pub fn map(&self, e: &Extents) -> Rect<Logical> {
        let x0 = self.origin_x + e.x as f64 * self.factor;
        let y0 = self.origin_y + e.y as f64 * self.factor;
        let x1 = self.origin_x + (e.x + e.w) as f64 * self.factor;
        let y1 = self.origin_y + (e.y + e.h) as f64 * self.factor;
        let (x0, y0) = (x0.round() as i32, y0.round() as i32);
        Rect::new(x0, y0, x1.round() as i32 - x0, y1.round() as i32 - y0)
    }
}

/// The widest border of a client area.
const MAX_BORDER: f64 = 16.0;

/// The tallest title bar above a client area.
const MAX_TITLE_BAR: f64 = 64.0;

/// Returns true when a reported length matches a compositor length within 2 units or 2 %.
fn close(reported: f64, actual: i32) -> bool {
    let tolerance = (actual as f64 * 0.02).max(2.0);
    (reported - actual as f64).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use fakeland_types::ProcessId;

    use super::*;

    fn window(scale: f64) -> WindowFrame {
        WindowFrame {
            id: WindowId(1),
            pid: Some(ProcessId(10)),
            app: Some(ProcessId(10)),
            title: "t".into(),
            app_id: "a".into(),
            geometry: Rect::new(1000, 100, 400, 300),
            surface: Rect::new(976, 76, 448, 348),
            scale,
            focused: true,
        }
    }

    fn ext(x: i32, y: i32, w: i32, h: i32) -> Extents {
        Extents {
            x,
            y,
            w,
            h,
            space: CoordSpace::Window,
        }
    }

    #[test]
    fn frame_with_geometry_size() {
        let m = Mapping::calibrate(Some(ext(0, 0, 400, 300)), &window(1.0), CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::Geometry);
        assert_eq!(m.map(&ext(10, 20, 50, 30)), Rect::new(1010, 120, 50, 30));
    }

    #[test]
    fn frame_with_shadow_size_counts_from_the_surface() {
        let m = Mapping::calibrate(Some(ext(0, 0, 448, 348)), &window(1.0), CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::Surface);
        assert_eq!(
            m.map(&ext(24, 24, 400, 300)),
            Rect::new(1000, 100, 400, 300)
        );
    }

    #[test]
    fn frame_offset_inside_the_reported_space() {
        // The frame sits at (24, 24) inside a shadowed surface and has the geometry size.
        let m = Mapping::calibrate(
            Some(ext(24, 24, 400, 300)),
            &window(1.0),
            CoordSpace::Screen,
        );
        assert_eq!(m.anchor, Anchor::Geometry);
        assert_eq!(m.map(&ext(34, 44, 10, 10)), Rect::new(1010, 120, 10, 10));
        assert_eq!(m.space, CoordSpace::Screen);
    }

    #[test]
    fn physical_pixels_scale_down() {
        let m = Mapping::calibrate(Some(ext(0, 0, 600, 450)), &window(1.5), CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::Geometry);
        assert!((m.factor - 1.0 / 1.5).abs() < 1e-9);
        assert_eq!(m.map(&ext(150, 300, 75, 30)), Rect::new(1100, 300, 50, 20));
    }

    #[test]
    fn client_area_inside_decorations() {
        let mut w = window(1.0);
        w.geometry = Rect::new(0, 0, 1280, 800);
        w.surface = w.geometry;
        // A 30 px title bar and 3 px borders around a 1274x767 client area.
        let m = Mapping::calibrate(Some(ext(0, 0, 1274, 767)), &w, CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::ClientArea);
        assert_eq!(m.map(&ext(910, 725, 80, 30)), Rect::new(913, 755, 80, 30));
        // At scale 2 in physical pixels.
        let mut w2 = w.clone();
        w2.scale = 2.0;
        let m = Mapping::calibrate(Some(ext(0, 0, 2548, 1534)), &w2, CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::ClientArea);
        assert_eq!(m.map(&ext(0, 0, 20, 20)), Rect::new(3, 30, 10, 10));
    }

    #[test]
    fn unknown_frame_size_is_relative_to_the_geometry() {
        let m = Mapping::calibrate(Some(ext(0, 0, 90, 90)), &window(1.0), CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::Assumed);
        assert_eq!(m.map(&ext(5, 5, 10, 10)), Rect::new(1005, 105, 10, 10));
        let none = Mapping::calibrate(None, &window(2.0), CoordSpace::Window);
        assert_eq!(none.anchor, Anchor::Assumed);
    }

    #[test]
    fn physical_subtree_below_logical_controls() {
        let mut w = window(1.5);
        w.geometry = Rect::new(100, 50, 853, 533);
        w.surface = w.geometry;
        let m = Mapping::calibrate(Some(ext(0, 0, 853, 533)), &w, CoordSpace::Window);
        assert_eq!(m.factor, 1.0);
        // The content panel is logical; the document below it is physical.
        let panel = ext(0, 86, 853, 447);
        let doc = ext(0, 130, 1280, 669);
        let d = m.descend(Some(panel), Some(doc));
        assert!((d.factor - 1.0 / 1.5).abs() < 1e-9);
        assert_eq!(d.map(&doc), Rect::new(100, 137, 853, 446));
        // A physical button below the document stays physical.
        let button = ext(150, 310, 300, 75);
        let b = d.descend(Some(doc), Some(button));
        assert_eq!(b.factor, d.factor);
        assert_eq!(b.map(&button), Rect::new(200, 257, 200, 50));
        // A long page switches too: only widths and the top-left corner count.
        let long = ext(0, 130, 1280, 9000);
        assert!(m.descend(Some(panel), Some(long)).factor < 1.0);
    }

    #[test]
    fn logical_children_stay_logical() {
        let m = Mapping::calibrate(Some(ext(0, 0, 400, 300)), &window(1.5), CoordSpace::Window);
        let parent = ext(0, 40, 400, 260);
        assert_eq!(
            m.descend(Some(parent), Some(ext(10, 50, 100, 30))).factor,
            1.0
        );
        // Wider content scrolled to the left is logical and overflows without fitting.
        assert_eq!(
            m.descend(Some(parent), Some(ext(-300, 40, 1000, 260)))
                .factor,
            1.0
        );
        assert_eq!(m.descend(None, Some(ext(0, 0, 1, 1))).factor, 1.0);
        let unscaled =
            Mapping::calibrate(Some(ext(0, 0, 400, 300)), &window(1.0), CoordSpace::Window);
        assert_eq!(
            unscaled
                .descend(Some(parent), Some(ext(0, 40, 600, 100)))
                .factor,
            1.0
        );
    }

    #[test]
    fn small_differences_match() {
        let m = Mapping::calibrate(Some(ext(0, 0, 402, 299)), &window(1.0), CoordSpace::Window);
        assert_eq!(m.anchor, Anchor::Geometry);
    }
}
