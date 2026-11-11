//! Errors of single actions.

use fakeland_scenario::ExpandError;
use thiserror::Error;

use crate::capture::CaptureError;
use crate::input::InjectError;
use crate::outputs::OutputError;
use crate::shell::{MatchError, WindowOpError};
use crate::waits::WaitError;

/// An action that failed.
#[derive(Debug, Error)]
pub enum ActionError {
    /// The input program is invalid.
    #[error(transparent)]
    Expand(#[from] ExpandError),
    /// An input event failed.
    #[error(transparent)]
    Inject(#[from] InjectError),
    /// A window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// No mapped window matches.
    #[error("no matching window")]
    NoWindow,
    /// A window operation failed.
    #[error(transparent)]
    Window(#[from] WindowOpError),
    /// An output change failed.
    #[error(transparent)]
    Output(#[from] OutputError),
    /// A capture failed.
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// A wait failed.
    #[error(transparent)]
    Wait(#[from] WaitError),
    /// The action has no meaning here, such as a nested repeat outside a scenario.
    #[error("{0} runs only as a scenario step")]
    Unsupported(&'static str),
}
