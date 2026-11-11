//! Resources: CPU, memory and thread counts of app process trees, their CPU time, wakeups, memory
//! detail and GPU use, device GPU use, and device energy, from the sample records of the trace.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Resources;
pub use report::{EnergyDomain, GpuResources, ProcessResources, ResourcesReport};
