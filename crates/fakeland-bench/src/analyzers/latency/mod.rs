//! Input latency from probe results: per-stage summaries with confidence intervals, the
//! distribution over the vblank phase of the input and a histogram of the total.

mod analyzer;
mod build;
mod phase;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Latency;
pub use phase::PhaseBin;
pub use report::{LatencyReport, LatencySample};
