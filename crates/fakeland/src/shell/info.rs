//! The public description of a toplevel window.

use fakeland_types::{Logical, OutputId, ProcessId, Rect, SurfaceId, Timestamp, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The placement state of a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// The window fills its output with the maximized state.
    Maximized,
    /// The window covers its output with the fullscreen state.
    Fullscreen,
    /// The window floats at its own geometry.
    Floating,
}

/// A toplevel window as the session sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowInfo {
    /// The window id.
    pub id: WindowId,
    /// The `xdg_toplevel` app id.
    pub app_id: String,
    /// The `xdg_toplevel` title.
    pub title: String,
    /// The pid of the client process.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
    /// The output that shows the window.
    pub output: OutputId,
    /// The window geometry in the global logical space.
    pub geometry: Rect<Logical>,
    /// The root surface.
    pub surface: SurfaceId,
    /// The time of the first buffer. `None` while the window is unmapped.
    pub mapped_at: Option<Timestamp>,
    /// The window has keyboard focus.
    pub focused: bool,
    /// The window carries the suspended state.
    pub suspended: bool,
    /// The placement state.
    pub placement: Placement,
}

impl WindowInfo {
    /// Returns true when the window shows a buffer.
    pub fn is_mapped(&self) -> bool {
        self.mapped_at.is_some()
    }
}
