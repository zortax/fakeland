//! Buffers: buffers in flight, allocation churn, formats and modifiers, sync kinds and whether
//! the buffer resolution fits the output scale.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Buffers;
pub use report::{BuffersReport, FormatCount, ScaleFit};
