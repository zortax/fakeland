//! Errors of session start and session calls.

use thiserror::Error;

use crate::actions::ActionError;
use crate::capture::CaptureError;
use crate::config::ConfigError;
use crate::input::InjectError;
use crate::launch::{DbusError, LaunchError};
use crate::outputs::OutputError;
use crate::render::RenderError;
use crate::shell::{MatchError, WindowOpError};
use crate::waits::WaitError;

/// A session that fails to start.
#[derive(Debug, Error)]
pub enum StartError {
    /// The config is invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The private bus does not start.
    #[error(transparent)]
    Dbus(#[from] DbusError),
    /// The renderer does not start.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// The socket does not open.
    #[error("cannot open the wayland socket: {0}")]
    Socket(String),
    /// The compositor thread or event loop does not start.
    #[error("compositor init failed: {0}")]
    Init(String),
    /// The viewer window does not start.
    #[error("viewer: {0}")]
    Viewer(String),
}

/// A session call that failed.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The session has shut down.
    #[error("the session has shut down")]
    Closed,
    /// A wait ran out of time.
    #[error("timed out")]
    Timeout,
    /// A window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// No mapped window matches.
    #[error("no matching window")]
    NoWindow,
    /// The app ended before it mapped a window.
    #[error("the app exited before it mapped a window")]
    AppExited,
    /// A launch failed.
    #[error(transparent)]
    Launch(#[from] LaunchError),
    /// An output change failed.
    #[error(transparent)]
    Output(#[from] OutputError),
    /// A window operation failed.
    #[error(transparent)]
    WindowOp(#[from] WindowOpError),
    /// A capture failed.
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// A trace capture id is unknown.
    #[error(transparent)]
    Trace(#[from] fakeland_trace::recorder::RecorderError),
    /// The clipboard is not readable.
    #[error("clipboard: {0}")]
    Clipboard(String),
    /// An input event failed.
    #[error(transparent)]
    Inject(#[from] InjectError),
    /// An action failed.
    #[error(transparent)]
    Action(#[from] ActionError),
    /// A wait failed.
    #[error(transparent)]
    Wait(#[from] WaitError),
    /// A key combination does not parse.
    #[error(transparent)]
    Combo(#[from] fakeland_scenario::ComboError),
    /// A frame capture id is unknown.
    #[error("no such frame capture")]
    NoFrameCapture,
    /// An accessibility call failed.
    #[error("accessibility: {0}")]
    A11y(#[from] fakeland_a11y::A11yError),
    /// A recording failed to start or stop.
    #[error(transparent)]
    Record(#[from] crate::video::RecordError),
}
