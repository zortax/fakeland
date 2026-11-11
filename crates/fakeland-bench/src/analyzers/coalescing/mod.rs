//! Input coalescing: how many injected input events each presented frame answers.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::InputCoalescing;
pub use report::CoalescingReport;
