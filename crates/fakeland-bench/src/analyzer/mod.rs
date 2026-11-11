//! The analyzer trait, its type-erased form, and combinators that run several analyzers on one
//! run.

mod combine;
mod erased;
mod set;
mod traits;

#[cfg(test)]
mod tests;

pub use combine::{AnalyzerExt, Both, BothReport, Named};
pub use erased::DynAnalyzer;
pub use set::AnalyzerSet;
pub use traits::Analyzer;
