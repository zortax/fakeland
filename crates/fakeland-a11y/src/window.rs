//! The compositor's description of a window: what the accessibility client needs to tie an app
//! frame to a window and to map its coordinates into the global logical space.

use fakeland_types::{Logical, ProcessId, Rect, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One mapped compositor window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowFrame {
    /// The window id.
    pub id: WindowId,
    /// The pid of the Wayland client.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
    /// The `xdg_toplevel` title.
    pub title: String,
    /// The `xdg_toplevel` app id.
    pub app_id: String,
    /// The window geometry in the global logical space. It excludes client-side shadows.
    pub geometry: Rect<Logical>,
    /// The bounding box of the root surface in the global logical space. It includes client-side
    /// shadows.
    pub surface: Rect<Logical>,
    /// The scale of the output that shows the window.
    pub scale: f64,
    /// The window has keyboard focus.
    pub focused: bool,
}

impl WindowFrame {
    /// Returns true when an accessible app with this pid owns the window.
    pub fn owned_by(&self, pid: ProcessId) -> bool {
        self.pid == Some(pid) || self.app == Some(pid)
    }
}
