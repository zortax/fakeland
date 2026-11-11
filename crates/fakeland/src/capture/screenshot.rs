//! Screenshot results and options. A screenshot maps its pixels back to the global logical space.

use fakeland_types::{Logical, OutputId, Point, Rect, Timestamp, WindowId};
use fakeland_vision::Image;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::shell::MatchError;

/// Options of a screenshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CaptureOptions {
    /// Draw the cursor.
    pub include_cursor: bool,
    /// Downscale so the longer side is at most this many pixels.
    pub max_dim: Option<u32>,
}

/// A captured image with its place in the global logical space.
#[derive(Clone, Debug, PartialEq)]
pub struct Screenshot {
    /// The pixels, RGBA, top row first.
    pub image: Image,
    /// The captured rectangle in the global logical space.
    pub region: Rect<Logical>,
    /// Logical units per image pixel.
    pub scale: f64,
    /// The output the region lies on.
    pub output: Option<OutputId>,
    /// The captured window.
    pub window: Option<WindowId>,
    /// The capture time.
    pub time: Timestamp,
}

impl Screenshot {
    /// Maps an image pixel position to the global logical space.
    pub fn to_logical(&self, x: f64, y: f64) -> Point<Logical> {
        Point::new(
            self.region.x as f64 + x * self.scale,
            self.region.y as f64 + y * self.scale,
        )
    }

    /// Downscales the image so its longer side is at most `max_dim` pixels.
    #[must_use]
    pub fn downscaled(mut self, max_dim: Option<u32>) -> Self {
        if let Some(max) = max_dim
            && self.image.width().max(self.image.height()) > max
        {
            let (image, factor) = self.image.downscale_to_max_dim(max);
            self.image = image;
            self.scale *= factor;
        }
        self
    }

    /// Maps a global logical position to image pixels.
    pub fn to_pixel(&self, p: Point<Logical>) -> (f64, f64) {
        (
            (p.x - self.region.x as f64) / self.scale,
            (p.y - self.region.y as f64) / self.scale,
        )
    }
}

/// A capture that failed.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// No output matches.
    #[error("no such output")]
    NoOutput,
    /// No mapped window matches.
    #[error("no such window")]
    NoWindow,
    /// The window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// The region is empty.
    #[error("the capture region is empty")]
    Empty,
    /// The renderer failed.
    #[error("render failed: {0}")]
    Render(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pixels_both_ways() {
        let shot = Screenshot {
            image: Image::new(10, 10),
            region: Rect::new(100, 50, 800, 600),
            scale: 0.5,
            output: None,
            window: None,
            time: Timestamp(0),
        };
        let p = shot.to_logical(20.0, 10.0);
        assert_eq!((p.x, p.y), (110.0, 55.0));
        assert_eq!(shot.to_pixel(p), (20.0, 10.0));
    }
}
