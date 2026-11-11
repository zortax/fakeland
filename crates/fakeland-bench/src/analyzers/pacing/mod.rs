//! Frame pacing: present intervals, jitter, missed vblanks, lows, jank, commit cadence against the
//! refresh, discarded frames with present mode detection, commit-timing adherence and a per-second
//! frame rate series.

mod analyzer;
mod build;
mod mode;
mod report;
mod samples;
mod series;

#[cfg(test)]
mod tests;

pub use analyzer::FramePacing;
pub use mode::PresentMode;
pub use report::{
    CadenceReport, CommitTimingReport, FpsBucket, FramePacingReport, JankFrame, JankReport,
};
pub use samples::{PacingFrame, PacingSamples};
