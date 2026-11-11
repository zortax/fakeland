//! Scene selection and the options of each scene.

use fakeland_types::{Logical, Rect};
use serde::Serialize;

use super::ScrollUnit;
use crate::draw::Color;

/// One scene that the client draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum SceneKind {
    /// A target rectangle that changes color while the pointer is inside.
    Hover,
    /// A rectangle that toggles its color on each press.
    Click,
    /// An endless list of numbered stripes that follows scroll events.
    Scroll,
    /// Typed characters as glyph boxes.
    Type,
    /// A bar that moves at constant speed.
    Animate,
    /// The window size as text, for configure tests.
    Resize,
    /// One frame, then frames only for configures.
    Idle,
}

/// Everything the scenes need to know.
#[derive(Clone, Debug)]
pub struct SceneOptions {
    /// The scenes to draw.
    pub kinds: Vec<SceneKind>,
    /// The background color.
    pub background: Color,
    /// The hover target.
    pub hover_rect: Rect<Logical>,
    /// The hover target color while the pointer is outside.
    pub hover_color: Color,
    /// The hover target color while the pointer is inside.
    pub hover_active_color: Color,
    /// The click target, or `None` for the whole window.
    pub click_rect: Option<Rect<Logical>>,
    /// The click target color after an even number of presses.
    pub click_color: Color,
    /// The click target color after an odd number of presses.
    pub click_active_color: Color,
    /// How scroll frames become an offset.
    pub scroll_unit: ScrollUnit,
    /// Logical pixels per wheel detent for the value120 unit.
    pub wheel_step: f64,
    /// The factor from scroll amount to content offset.
    pub scroll_factor: f64,
    /// Logical stripe height.
    pub stripe_height: f64,
    /// Bar speed in logical pixels per second.
    pub speed: f64,
    /// Logical bar width.
    pub bar_width: f64,
    /// Bar color.
    pub bar_color: Color,
    /// Draw a marker at the pointer position.
    pub pointer_marker: bool,
    /// Draw the frame id in the bottom-left corner.
    pub show_frame_id: bool,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            kinds: vec![SceneKind::Hover],
            background: Color::rgb(0x20, 0x20, 0x20),
            hover_rect: Rect::new(100, 100, 200, 150),
            hover_color: Color::rgb(0x30, 0x60, 0xc0),
            hover_active_color: Color::rgb(0xe0, 0x60, 0x20),
            click_rect: None,
            click_color: Color::rgb(0x20, 0x40, 0x20),
            click_active_color: Color::rgb(0xc0, 0xc0, 0x30),
            scroll_unit: ScrollUnit::Pixels,
            wheel_step: 15.0,
            scroll_factor: 1.0,
            stripe_height: 40.0,
            speed: 600.0,
            bar_width: 40.0,
            bar_color: Color::rgb(0xf0, 0xf0, 0xf0),
            pointer_marker: false,
            show_frame_id: false,
        }
    }
}

impl SceneOptions {
    /// Returns true when the scene is selected.
    pub fn has(&self, kind: SceneKind) -> bool {
        self.kinds.contains(&kind)
    }
}
