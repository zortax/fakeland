//! Coordinate mapping between screenshot pixels, the global logical space and outputs.

use fakeland::OutputState;
use fakeland_types::{Logical, OutputId, Rect, WindowId};

/// The pixel frame of the last screenshot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotFrame {
    /// The captured rectangle in the global logical space.
    pub region: Rect<Logical>,
    /// Logical units per image pixel.
    pub scale: f64,
    /// The output of the region.
    pub output: Option<OutputId>,
    /// The captured window.
    pub window: Option<WindowId>,
    /// The image size in pixels.
    pub size: (u32, u32),
}

impl ShotFrame {
    /// Maps an image pixel to the global logical space.
    pub fn to_global(self, px: f64, py: f64) -> (f64, f64) {
        (
            f64::from(self.region.x) + px * self.scale,
            f64::from(self.region.y) + py * self.scale,
        )
    }

    /// Returns true when a pixel lies inside the image.
    pub fn contains(self, px: f64, py: f64) -> bool {
        px >= 0.0 && py >= 0.0 && px < f64::from(self.size.0) && py < f64::from(self.size.1)
    }
}

/// An output area in the global logical space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputArea {
    /// The output.
    pub id: OutputId,
    /// The left edge.
    pub x: f64,
    /// The top edge.
    pub y: f64,
    /// The width.
    pub w: f64,
    /// The height.
    pub h: f64,
}

impl OutputArea {
    /// Builds the area of an output.
    pub fn of(o: &OutputState) -> Self {
        Self {
            id: o.id,
            x: f64::from(o.position.x),
            y: f64::from(o.position.y),
            w: f64::from(o.logical_size.w),
            h: f64::from(o.logical_size.h),
        }
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Maps a global logical point to coordinates on the output that contains it. A point outside
/// every output goes to the preferred output, or the first one.
pub fn global_to_output(
    outputs: &[OutputArea],
    x: f64,
    y: f64,
    prefer: Option<OutputId>,
) -> Option<(OutputId, f64, f64)> {
    let area = outputs
        .iter()
        .find(|o| o.contains(x, y))
        .or_else(|| prefer.and_then(|id| outputs.iter().find(|o| o.id == id)))
        .or_else(|| outputs.first())?;
    Some((area.id, x - area.x, y - area.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> ShotFrame {
        ShotFrame {
            region: Rect::new(1280, 0, 1920, 1080),
            scale: 1.5,
            output: Some(OutputId(2)),
            window: None,
            size: (1280, 720),
        }
    }

    #[test]
    fn maps_pixels_to_outputs() {
        let outputs = [
            OutputArea {
                id: OutputId(1),
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 720.0,
            },
            OutputArea {
                id: OutputId(2),
                x: 1280.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0,
            },
        ];
        let f = frame();
        let (gx, gy) = f.to_global(100.0, 200.0);
        assert_eq!((gx, gy), (1430.0, 300.0));
        assert_eq!(
            global_to_output(&outputs, gx, gy, None),
            Some((OutputId(2), 150.0, 300.0))
        );
        assert_eq!(
            global_to_output(&outputs, -5.0, 10.0, Some(OutputId(2))),
            Some((OutputId(2), -1285.0, 10.0))
        );
        assert!(f.contains(0.0, 0.0));
        assert!(!f.contains(1280.0, 10.0));
        assert_eq!(global_to_output(&[], 1.0, 1.0, None), None);
    }
}
