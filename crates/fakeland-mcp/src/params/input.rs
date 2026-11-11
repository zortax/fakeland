//! Parameters of the pointer and keyboard tools. Coordinates are pixels of the last screenshot
//! unless `logical` is set or `window` names a window.

use schemars::JsonSchema;
use serde::Deserialize;

use super::defaults;

/// A point.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PointParams {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
    /// The coordinates are logical output coordinates, not screenshot pixels.
    #[serde(default)]
    pub logical: bool,
    /// The coordinates are logical and relative to this window.
    #[serde(default)]
    pub window: Option<String>,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// A click.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClickParams {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
    /// The button: left, right, middle, side, extra, forward, back.
    #[serde(default)]
    pub button: Option<String>,
    /// The count of clicks.
    #[serde(default = "defaults::one")]
    pub count: u32,
    /// The coordinates are logical output coordinates, not screenshot pixels.
    #[serde(default)]
    pub logical: bool,
    /// The coordinates are logical and relative to this window.
    #[serde(default)]
    pub window: Option<String>,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// A drag along points with the left button held.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DragParams {
    /// The points as `[x, y]` pairs: start, optional middle points, end.
    pub path: Vec<[f64; 2]>,
    /// The drag time in milliseconds.
    #[serde(default = "defaults::drag_ms")]
    pub duration_ms: u64,
    /// The held button.
    #[serde(default)]
    pub button: Option<String>,
    /// The coordinates are logical output coordinates, not screenshot pixels.
    #[serde(default)]
    pub logical: bool,
    /// The coordinates are logical and relative to this window.
    #[serde(default)]
    pub window: Option<String>,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// A scroll. Positive `dy` scrolls down, positive `dx` scrolls right, in logical pixels.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScrollParams {
    /// Move the pointer here first.
    #[serde(default)]
    pub x: Option<f64>,
    /// Move the pointer here first.
    #[serde(default)]
    pub y: Option<f64>,
    /// Horizontal distance.
    #[serde(default)]
    pub dx: f64,
    /// Vertical distance.
    #[serde(default)]
    pub dy: f64,
    /// Spread the distance over this many milliseconds. Without it, one event.
    #[serde(default)]
    pub smooth_ms: Option<u64>,
    /// Scroll as a classic wheel with whole detents of 15 px.
    #[serde(default)]
    pub detents: bool,
    /// Scroll as two fingers on a touchpad.
    #[serde(default)]
    pub finger: bool,
    /// The coordinates are logical output coordinates, not screenshot pixels.
    #[serde(default)]
    pub logical: bool,
    /// The coordinates are logical and relative to this window.
    #[serde(default)]
    pub window: Option<String>,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// Text to type into the focused window.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeTextParams {
    /// The text. Newlines press Return.
    pub text: String,
    /// Words per minute.
    #[serde(default = "defaults::wpm")]
    pub wpm: f64,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// A key or combination.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyParams {
    /// The key or combination: `Return`, `Escape`, `Tab`, `BackSpace`, `Down`, `ctrl+c`,
    /// `ctrl+shift+t`, `alt+F4`, `super`. Key names are xkb keysym names.
    pub combo: String,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}
