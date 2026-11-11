//! Errors of the recorder.

use crate::recorder::CaptureId;

/// An error of the recorder.
#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    /// The capture is not open.
    #[error("{0} is not open")]
    UnknownCapture(CaptureId),
}
