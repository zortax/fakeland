//! Window methods: listing, waits and window operations.

use fakeland::{ClientInfo, WaitOutput, WindowInfo, WindowOp};
use fakeland_scenario::{Wait, WindowRef};
use fakeland_types::{Dur, Timestamp};
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::{NoParams, method};

/// A wait for a mapped window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaitWindowParams {
    /// The window to wait for. The default matches any window.
    #[serde(default = "any_window")]
    pub window: WindowRef,
    /// The longest wait.
    #[serde(default = "defaults::wait_timeout")]
    pub timeout: Dur,
}

/// A wait for a window whose content stops changing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaitSettledParams {
    /// The window. The default is the focused window.
    #[serde(default)]
    pub window: WindowRef,
    /// The time without a content change.
    #[serde(default = "defaults::quiet")]
    pub quiet: Dur,
    /// The longest wait.
    #[serde(default = "defaults::wait_timeout")]
    pub timeout: Dur,
}

/// A wait for presented frames of a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaitFramesParams {
    /// The window. The default is the focused window.
    #[serde(default)]
    pub window: WindowRef,
    /// The count of frames.
    #[serde(default = "defaults::one")]
    pub n: u32,
    /// The longest wait.
    #[serde(default = "defaults::wait_timeout")]
    pub timeout: Dur,
}

/// One operation on a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowOpParams {
    /// The window. The default is the focused window.
    #[serde(default)]
    pub window: WindowRef,
    /// The operation.
    pub op: WindowOp,
}

/// Returns the matcher for any window.
pub fn any_window() -> WindowRef {
    WindowRef::Any
}

method!(
    /// Returns every window, mapped ones first.
    Windows, "windows", NoParams => Vec<WindowInfo>
);
method!(
    /// Returns every connected client.
    Clients, "clients", NoParams => Vec<ClientInfo>
);
method!(
    /// Waits for a mapped window.
    WaitWindow, "wait_window", WaitWindowParams => WindowInfo
);
method!(
    /// Waits until a window shows no new content for the quiet time.
    WaitSettled, "wait_settled", WaitSettledParams => WindowInfo
);
method!(
    /// Waits for presented frames and returns the last presentation time.
    WaitFrames, "wait_frames", WaitFramesParams => Timestamp
);
method!(
    /// Runs any scenario wait on the compositor thread.
    WaitFor, "wait", Wait => WaitOutput
);
method!(
    /// Runs a window operation.
    WindowOperation, "window_op", WindowOpParams => ()
);
