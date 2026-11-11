//! The surfaces, windows, clients and processes of a trace.

use fakeland_types::{ClientId, OutputId, ProcessId, SurfaceId, Timestamp, WindowId};
use serde::{Deserialize, Serialize};

/// A surface and its role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceInfo {
    /// The surface.
    pub surface: SurfaceId,
    /// The owner.
    pub client: Option<ClientId>,
    /// The role, for example `xdg_toplevel` or `subsurface`.
    pub role: Option<String>,
    /// The parent of a subsurface or popup.
    pub parent: Option<SurfaceId>,
    /// The creation time.
    pub created: Option<Timestamp>,
    /// The destruction time.
    pub destroyed: Option<Timestamp>,
}

impl SurfaceInfo {
    /// Builds the info of a surface that the trace names before it creates it.
    pub const fn unknown(surface: SurfaceId) -> Self {
        Self {
            surface,
            client: None,
            role: None,
            parent: None,
            created: None,
            destroyed: None,
        }
    }
}

/// A toplevel window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// The window.
    pub window: WindowId,
    /// The toplevel surface.
    pub surface: SurfaceId,
    /// The latest app id.
    pub app_id: String,
    /// The latest title.
    pub title: String,
    /// The output that shows the window.
    pub output: Option<OutputId>,
    /// The map time.
    pub mapped: Timestamp,
    /// The unmap time.
    pub unmapped: Option<Timestamp>,
}

/// A Wayland client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// The client.
    pub client: ClientId,
    /// The client process.
    pub pid: Option<ProcessId>,
    /// The connect time.
    pub connected: Option<Timestamp>,
    /// The disconnect time.
    pub disconnected: Option<Timestamp>,
}

/// A process that the launcher starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// The process.
    pub pid: ProcessId,
    /// The command line.
    pub command: String,
    /// The spawn time.
    pub spawned: Timestamp,
    /// The exit time.
    pub exited: Option<Timestamp>,
    /// The exit code of a normal exit.
    pub code: Option<i32>,
    /// The signal that ended the process.
    pub signal: Option<i32>,
}
