//! Why the client exits and the process exit code of each reason.

use std::fmt;

/// The process exit code of a command-line usage error.
pub const USAGE_EXIT_CODE: i32 = 2;

/// Why the client exits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExitReason {
    /// The compositor closed the toplevel.
    Closed,
    /// The `--exit-after` time passed.
    Timeout,
    /// The `--exit-after-frames` frames are committed and their feedback arrived.
    FramesDone,
    /// The connection failed or the compositor went away.
    Disconnected(String),
    /// The compositor lacks a global that the options need.
    MissingGlobal(String),
    /// The compositor raised a protocol error.
    Protocol(String),
    /// A local failure, such as a buffer that cannot be allocated.
    Failed(String),
}

impl ExitReason {
    /// Returns the process exit code: 0 for a normal end, 1 for a local failure, 3 for a lost
    /// connection, 4 for a missing global and 5 for a protocol error.
    pub fn code(&self) -> i32 {
        match self {
            Self::Closed | Self::Timeout | Self::FramesDone => 0,
            Self::Failed(_) => 1,
            Self::Disconnected(_) => 3,
            Self::MissingGlobal(_) => 4,
            Self::Protocol(_) => 5,
        }
    }
}

impl fmt::Display for ExitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("closed"),
            Self::Timeout => f.write_str("timeout"),
            Self::FramesDone => f.write_str("frames done"),
            Self::Disconnected(e) => write!(f, "disconnected: {e}"),
            Self::MissingGlobal(g) => write!(f, "missing global: {g}"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::Failed(e) => write!(f, "failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(ExitReason::Closed.code(), 0);
        assert_eq!(ExitReason::Timeout.code(), 0);
        assert_eq!(ExitReason::FramesDone.code(), 0);
        assert_eq!(ExitReason::Disconnected("x".into()).code(), 3);
        assert_eq!(ExitReason::MissingGlobal("wl_shm".into()).code(), 4);
        assert_eq!(ExitReason::Protocol("x".into()).code(), 5);
        assert_eq!(ExitReason::Failed("x".into()).code(), 1);
        assert_eq!(
            ExitReason::MissingGlobal("wl_shm".into()).to_string(),
            "missing global: wl_shm"
        );
    }
}
