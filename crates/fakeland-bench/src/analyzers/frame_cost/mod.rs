//! Frame cost: the CPU time, GPU fence time and damage of each frame of the target's main surface.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::{FULL_DAMAGE, FrameCost};
pub use report::FrameCostReport;
