//! Session-wide methods: information, validity, marks, manual steps and shutdown.

use fakeland::{RunValidity, SessionConfig};
use fakeland_trace::Environment;
use fakeland_types::OutputId;
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::{NoParams, method};

/// Facts about the running session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The wire protocol version of the server.
    pub protocol: u32,
    /// The value for `WAYLAND_DISPLAY`.
    pub wayland_display: String,
    /// The runtime directory that holds the Wayland socket.
    pub runtime_dir: Option<String>,
    /// The private bus address, when the session has one.
    pub dbus_address: Option<String>,
    /// The pid of the server process.
    pub server_pid: u32,
    /// The host environment of the trace header.
    pub environment: Environment,
    /// The session config.
    pub config: SessionConfig,
}

/// A user mark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkParams {
    /// The label of the mark.
    pub label: String,
}

/// A manual step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepParams {
    /// The output. `None` selects the primary output.
    #[serde(default)]
    pub output: Option<OutputId>,
    /// The count of refreshes.
    #[serde(default = "defaults::one")]
    pub n: u32,
}

method!(
    /// Returns [`SessionInfo`].
    Info, "info", NoParams => SessionInfo
);
method!(
    /// Returns the compositor lateness of the session so far.
    Validity, "validity", NoParams => RunValidity
);
method!(
    /// Clears the lateness record, for example after a warmup.
    ResetValidity, "reset_validity", NoParams => ()
);
method!(
    /// Records a user mark in the trace.
    Mark, "mark", MarkParams => ()
);
method!(
    /// Steps an output with manual timing.
    Step, "step", StepParams => ()
);
method!(
    /// Shuts the session down. Children end, and the server stops.
    Shutdown, "shutdown", NoParams => ()
);
