//! Plain-text tables for people: trace summaries, run validity, windows and run summaries.

mod probe;
mod runs;
mod summary;
mod windows;

pub use probe::probe_text;
pub use runs::run_text;
pub use summary::{summary_text, validity_text};
pub use windows::{outputs_text, windows_text};
