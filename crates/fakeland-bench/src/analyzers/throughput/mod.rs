//! Throughput: the commit and present rates of an unthrottled client and its GPU time from commit
//! to fence ready.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Throughput;
pub use report::ThroughputReport;
