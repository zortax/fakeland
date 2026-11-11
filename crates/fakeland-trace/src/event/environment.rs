//! The host environment of a session. Benchmark results are comparable only on equal hosts.

use serde::{Deserialize, Serialize};

/// A description of the host that recorded a trace. An empty field means unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Environment {
    /// The GPU model.
    pub gpu: String,
    /// The GPU driver name and version.
    pub driver: String,
    /// The kernel release.
    pub kernel: String,
    /// The CPU model.
    pub cpu: String,
    /// The CPU frequency governor.
    pub governor: String,
    /// The compositor renderer, for example `gles` or `pixman`.
    pub renderer: String,
    /// The host name.
    pub hostname: String,
    /// The fakeland version.
    pub fakeland_version: String,
}
