//! Resources: CPU, memory and thread counts of app processes and GPU use, from the sample records
//! of the trace.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Resources;
pub use report::{GpuResources, ProcessResources, ResourcesReport};
