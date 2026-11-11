//! App methods: launch, list, signal, restart and read output.

use fakeland::{ExitInfo, LaunchSpec, WindowInfo};
use fakeland_types::{Dur, ProcessId};
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::{NoParams, method};

/// A launch request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LaunchParams {
    /// What to launch.
    #[serde(flatten)]
    pub spec: LaunchSpec,
    /// Waits this long for the first mapped window of the app. `None` returns at once.
    #[serde(default)]
    pub wait_window: Option<Dur>,
}

/// An app that the server launched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppInfo {
    /// The pid, which is also the process group.
    pub pid: ProcessId,
    /// The command line.
    pub command: String,
    /// The exit status once the app ended.
    pub exit: Option<ExitInfo>,
    /// The first mapped window, when the call waited for one.
    #[serde(default)]
    pub window: Option<WindowInfo>,
}

impl AppInfo {
    /// Returns true while the app runs.
    pub fn running(&self) -> bool {
        self.exit.is_none()
    }
}

/// The signal for [`Kill`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillSignal {
    /// SIGTERM.
    #[default]
    Term,
    /// SIGKILL.
    Kill,
}

/// A signal to an app.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct KillParams {
    /// The app. `None` selects the newest app.
    #[serde(default)]
    pub pid: Option<ProcessId>,
    /// The signal.
    #[serde(default)]
    pub signal: KillSignal,
    /// Waits this long for the exit. After a SIGTERM that times out, the app gets SIGKILL.
    #[serde(default)]
    pub wait: Option<Dur>,
}

/// A restart: the app ends and starts again with the same command.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestartParams {
    /// The app. `None` selects the newest app.
    #[serde(default)]
    pub pid: Option<ProcessId>,
    /// Waits this long for the first window of the new process.
    #[serde(default)]
    pub wait_window: Option<Dur>,
}

/// A request for the captured output of an app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogsParams {
    /// The app. `None` selects the newest app.
    #[serde(default)]
    pub pid: Option<ProcessId>,
    /// The count of last lines of each stream.
    #[serde(default = "defaults::log_lines")]
    pub lines: usize,
}

impl Default for LogsParams {
    fn default() -> Self {
        Self {
            pid: None,
            lines: defaults::log_lines(),
        }
    }
}

/// The tails of stdout and stderr of an app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Logs {
    /// The app.
    pub app: AppInfo,
    /// The last lines of stdout.
    pub stdout: String,
    /// The last lines of stderr.
    pub stderr: String,
}

method!(
    /// Launches an app inside the session.
    Launch, "launch", LaunchParams => AppInfo
);
method!(
    /// Returns every app the server launched.
    Apps, "apps", NoParams => Vec<AppInfo>
);
method!(
    /// Sends a signal to the process group of an app.
    Kill, "kill", KillParams => AppInfo
);
method!(
    /// Ends an app and launches its command again.
    Restart, "restart", RestartParams => AppInfo
);
method!(
    /// Returns the last lines of stdout and stderr of an app.
    AppLogs, "logs", LogsParams => Logs
);
