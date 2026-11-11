//! Parameters of the session, app, window, output and clipboard tools.

use std::collections::BTreeMap;

use fakeland_control::SessionSpec;
use schemars::JsonSchema;
use serde::Deserialize;

use super::defaults;

/// No parameters.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct NoArgs {}

/// Session settings. Every field is optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SessionStartParams {
    /// The settings.
    #[serde(flatten)]
    pub spec: SessionSpec,
    /// Stop a running session first. Without it, a running session is an error.
    #[serde(default)]
    pub restart: bool,
}

/// An app to launch.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LaunchParams {
    /// The program, such as `alacritty`. Without `args`, a command line with spaces is split
    /// at whitespace.
    pub command: String,
    /// The arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// More environment variables.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// The working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Wait this long for the first window, in milliseconds. 0 returns at once.
    #[serde(default = "defaults::wait_ms")]
    pub wait_window_ms: u64,
}

/// A window.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowParams {
    /// The window: `focused`, `any`, a window id such as `3`, `app:APP_ID` or `title:REGEX`.
    pub window: String,
}

/// A request for app output.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogsParams {
    /// The app pid. The default is the newest app.
    #[serde(default)]
    pub pid: Option<u32>,
    /// The count of last lines of stdout and of stderr.
    #[serde(default = "defaults::lines")]
    pub lines: usize,
}

/// An app to stop.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KillAppParams {
    /// The app pid. The default is the newest app.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Send SIGKILL instead of SIGTERM.
    #[serde(default)]
    pub force: bool,
}

/// An app to restart with its original command.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestartAppParams {
    /// The app pid. The default is the newest app.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Wait this long for the first window, in milliseconds.
    #[serde(default = "defaults::wait_ms")]
    pub wait_window_ms: u64,
}

/// Output changes. Give at least one.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetOutputParams {
    /// The output id. The default is the primary output.
    #[serde(default)]
    pub output: Option<u32>,
    /// A mode as `WIDTHxHEIGHT[@HZ]`, such as `2560x1440@144`.
    #[serde(default)]
    pub mode: Option<String>,
    /// A scale, such as 1.5.
    #[serde(default)]
    pub scale: Option<f64>,
    /// A timing model: `fixed`, `vrr:MIN-MAX`, `unthrottled`, `manual` or `manual:HZ`.
    #[serde(default)]
    pub timing: Option<String>,
    /// Power the output on or off.
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Clipboard text.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipboardSetParams {
    /// The text.
    pub text: String,
}
