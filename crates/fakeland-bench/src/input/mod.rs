//! What an analyzer sees: the trace of one iteration with its frame timeline, the target window,
//! probe reports, frame captures, the scenario run and the iteration metadata.

mod analysis;
mod iteration;
mod run;
mod target;

pub use analysis::AnalysisInput;
pub use iteration::{IterationMeta, Observation};
pub use run::RunSummary;
pub use target::Target;
