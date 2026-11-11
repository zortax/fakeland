//! The screenshot method.

use std::path::PathBuf;

use fakeland_scenario::CaptureTarget;
use fakeland_types::{Logical, OutputId, Rect, Timestamp, WindowId};
use serde::{Deserialize, Serialize};

use super::method::method;

/// A screenshot request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenshotParams {
    /// What to capture. The default is the primary output.
    pub target: CaptureTarget,
    /// Draw the cursor.
    pub include_cursor: bool,
    /// Downscale so the longer side is at most this many pixels.
    pub max_dim: Option<u32>,
    /// Write the PNG to this absolute path on the server host.
    pub path: Option<PathBuf>,
    /// Return the PNG inline as base64.
    pub inline: bool,
}

/// A screenshot and its place in the global logical space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShotInfo {
    /// The image width in pixels.
    pub width: u32,
    /// The image height in pixels.
    pub height: u32,
    /// The captured rectangle in the global logical space.
    pub region: Rect<Logical>,
    /// Logical units per image pixel. A logical point is `region.origin + pixel * scale`.
    pub scale: f64,
    /// The output the region lies on.
    pub output: Option<OutputId>,
    /// The captured window.
    pub window: Option<WindowId>,
    /// The compositor time of the capture.
    pub time: Timestamp,
    /// The file the PNG went to.
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// The PNG as base64.
    #[serde(default)]
    pub png_base64: Option<String>,
}

method!(
    /// Captures an output, a window or a region.
    Screenshot, "screenshot", ScreenshotParams => ShotInfo
);
