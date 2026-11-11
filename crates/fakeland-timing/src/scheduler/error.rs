//! Errors of scheduler calls that do not fit the output state.

/// A scheduler call that the output state rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    /// `step` needs manual mode.
    #[error("the output does not run in manual mode")]
    NotManual,
    /// The output is off.
    #[error("the output is disabled")]
    Disabled,
}
