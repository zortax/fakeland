//! Resize and reconfigure: configure to ack to the first commit at the new size and its
//! presentation, frames whose size does not match the acked configure, and frames dropped during
//! a resize storm.

mod analyzer;
mod configures;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Resize;
pub use configures::{ConfigureRecord, configures_of};
pub use report::ResizeReport;
