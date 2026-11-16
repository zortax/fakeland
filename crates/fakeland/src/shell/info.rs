//! The public description of a window: a toplevel, a layer surface or a lock surface.

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

/// The shell role of a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// An `xdg_toplevel`.
    #[default]
    Toplevel,
    /// A `zwlr_layer_surface_v1`.
    Layer,
    /// An `ext_session_lock_surface_v1`.
    Lock,
}

/// The layer of a layer surface, bottom to top.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ShellLayer {
    /// Below every other surface.
    Background,
    /// Below the toplevels.
    Bottom,
    /// Above the toplevels.
    Top,
    /// Above every other surface.
    Overlay,
}

/// How a layer surface takes keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardMode {
    /// The surface never takes keyboard focus.
    None,
    /// The surface holds keyboard focus while it is mapped on the top or overlay layer.
    Exclusive,
    /// The surface takes keyboard focus when it maps on the top or overlay layer and when the
    /// user clicks it.
    OnDemand,
}

/// The layer-shell state of a layer surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayerInfo {
    /// The namespace that the client gave.
    pub namespace: String,
    /// The current layer.
    pub layer: ShellLayer,
    /// The keyboard interactivity.
    pub keyboard: KeyboardMode,
    /// The exclusive zone: positive reserves space, 0 is neutral, -1 ignores other zones.
    pub exclusive_zone: i32,
    /// The anchored edges among `top`, `bottom`, `left` and `right`.
    pub anchor: Vec<String>,
}

/// A window as the session sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowInfo {
    /// The window id.
    pub id: WindowId,
    /// The `xdg_toplevel` app id. Layer and lock surfaces have an empty app id.
    pub app_id: String,
    /// The `xdg_toplevel` title. Layer and lock surfaces have an empty title.
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
    /// The placement state. Layer surfaces float and lock surfaces are fullscreen.
    pub placement: Placement,
    /// The shell role.
    #[serde(default)]
    pub kind: WindowKind,
    /// The layer-shell state of a layer surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<LayerInfo>,
}

impl WindowInfo {
    /// Returns true when the window shows a buffer.
    pub fn is_mapped(&self) -> bool {
        self.mapped_at.is_some()
    }

    /// Returns the namespace of a layer surface.
    pub fn namespace(&self) -> Option<&str> {
        self.layer.as_ref().map(|l| l.namespace.as_str())
    }
}
