//! The window under test.

use fakeland::WindowInfo;
use fakeland_types::{Logical, OutputId, ProcessId, Rect, SurfaceId, WindowId};
use serde::{Deserialize, Serialize};

/// The window under test, as the compositor saw it when the iteration started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// The window.
    pub window: WindowId,
    /// The toplevel surface.
    pub surface: SurfaceId,
    /// The output of the window.
    pub output: OutputId,
    /// The window geometry in the global logical space.
    pub geometry: Rect<Logical>,
    /// The app id.
    pub app_id: String,
    /// The pid of the client process.
    #[serde(default)]
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    #[serde(default)]
    pub app: Option<ProcessId>,
}

impl From<&WindowInfo> for Target {
    fn from(w: &WindowInfo) -> Self {
        Self {
            window: w.id,
            surface: w.surface,
            output: w.output,
            geometry: w.geometry,
            app_id: w.app_id.clone(),
            pid: w.pid,
            app: w.app,
        }
    }
}
