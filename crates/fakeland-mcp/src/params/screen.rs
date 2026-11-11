//! Parameters of the screenshot and wait tools.

use fakeland_scenario::PixelCondition;
use schemars::JsonSchema;
use serde::Deserialize;

use super::defaults;

/// A rectangle in logical coordinates.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
pub struct Region {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// A screenshot request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Capture only this window: `focused`, `any`, an id, `app:APP_ID` or `title:REGEX`.
    #[serde(default)]
    pub window: Option<String>,
    /// Capture this output id. The default is the primary output.
    #[serde(default)]
    pub output: Option<u32>,
    /// Capture only this logical rectangle: relative to the window with `window`, else to the
    /// output.
    #[serde(default)]
    pub region: Option<Region>,
    /// Downscale so the longer side is at most this many pixels. 0 keeps full resolution.
    #[serde(default = "defaults::max_dim")]
    pub max_dim: u32,
    /// Draw the pointer.
    #[serde(default)]
    pub cursor: bool,
    /// First wait up to 1 s until no new frame came for this many milliseconds. 0 skips the
    /// wait.
    #[serde(default = "defaults::settle_ms")]
    pub settle_ms: u64,
}

/// What to wait for.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitKind {
    /// A mapped window that matches `window` (default any).
    Window,
    /// No new content in `window` for `quiet_ms`.
    Settled,
    /// `frames` presented frames of `window`.
    Frames,
    /// `region` shows the same pixels for `frames` refreshes.
    PixelsStable,
    /// `region` satisfies `condition`.
    PixelsMatch,
}

/// A wait on the compositor.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitForParams {
    /// What to wait for.
    pub until: WaitKind,
    /// The window: `focused`, `any`, an id, `app:APP_ID` or `title:REGEX`.
    #[serde(default)]
    pub window: Option<String>,
    /// The longest wait in milliseconds.
    #[serde(default = "defaults::wait_ms")]
    pub timeout_ms: u64,
    /// The quiet time of `settled` in milliseconds.
    #[serde(default = "defaults::quiet_ms")]
    pub quiet_ms: u64,
    /// The frame count of `frames` and `pixels_stable`.
    #[serde(default = "defaults::one")]
    pub frames: u32,
    /// The region of the pixel waits, in logical coordinates relative to the window with
    /// `window`, else to the primary output. The default is the whole window or output.
    #[serde(default)]
    pub region: Option<Region>,
    /// The pixel condition of `pixels_match`, such as
    /// `{"type": "mean_color", "rgba": "#ff8000"}` or
    /// `{"type": "color_at", "x": 10, "y": 10, "rgba": "#ffffff"}` or `{"type": "changed"}`.
    #[serde(default)]
    pub condition: Option<PixelCondition>,
}
