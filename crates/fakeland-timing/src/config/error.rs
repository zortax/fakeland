//! Errors of an invalid timing configuration.

/// A timing configuration that the scheduler cannot run.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ConfigError {
    /// A refresh rate is zero, negative or not finite.
    #[error("refresh rate {0} Hz is not a positive finite number")]
    InvalidRate(f64),
    /// The VRR range has a minimum above its maximum.
    #[error("VRR range {min} Hz to {max} Hz has a minimum above its maximum")]
    InvertedVrrRange {
        /// The lowest rate.
        min: f64,
        /// The highest rate.
        max: f64,
    },
    /// The latch offset is negative or does not fit into one refresh period.
    #[error("latch offset {offset_ns} ns does not fit into the period of {period_ns} ns")]
    LatchOffset {
        /// The configured offset.
        offset_ns: i64,
        /// The refresh period.
        period_ns: i64,
    },
    /// The callback lead is negative.
    #[error("callback lead {0} ns is negative")]
    NegativeLead(i64),
    /// The latch jitter is negative.
    #[error("latch jitter {0} ns is negative")]
    NegativeJitter(i64),
    /// The drop probability is outside `0.0..1.0`.
    #[error("drop probability {0} is outside 0.0..1.0")]
    DropProbability(f64),
    /// The drop interval drops every vblank.
    #[error("drop interval {0} drops too many vblanks; use 2 or more")]
    DropEvery(u32),
}
