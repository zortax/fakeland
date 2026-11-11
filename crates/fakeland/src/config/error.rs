//! Errors of session configuration checks.

use thiserror::Error;

/// A session config that cannot start.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The config has no output.
    #[error("a session needs at least one output")]
    NoOutputs,
    /// An output has an empty connector name.
    #[error("an output has an empty name")]
    OutputName,
    /// Two outputs share a connector name.
    #[error("two outputs share the name {0}")]
    DuplicateOutput(String),
    /// A mode has a non-positive size or refresh.
    #[error("output {output} has a mode with a bad size or refresh rate")]
    Mode {
        /// The output name.
        output: String,
    },
    /// A scale is outside 0.25..=8.
    #[error("output {output} has scale {scale}, outside 0.25..=8")]
    Scale {
        /// The output name.
        output: String,
        /// The rejected scale.
        scale: f64,
    },
    /// A timing config is invalid.
    #[error("output {output} has invalid timing: {source}")]
    Timing {
        /// The output name.
        output: String,
        /// The timing error.
        source: fakeland_timing::ConfigError,
    },
    /// The socket name is empty or contains a slash.
    #[error("bad socket name {0:?}")]
    SocketName(String),
    /// A span setting is negative.
    #[error("lateness threshold and timer spin must not be negative")]
    NegativeSpan,
    /// The trace capacity is zero.
    #[error("the trace capacity must be positive")]
    TraceCapacity,
    /// The floating window size is not positive.
    #[error("the floating window size must be positive")]
    WindowSize,
    /// The pointer report rate is not a positive finite rate.
    #[error("the pointer report rate must be positive")]
    ReportRate,
    /// The resource sample interval is shorter than 10 ms.
    #[error("the sample interval must be at least 10 ms")]
    SampleInterval,
}
