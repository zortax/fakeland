//! Traces: a header and the records of a session or a capture span, with slicing, filtering and
//! JSON and JSONL serialization.

mod error;
mod filter;
mod header;
mod json;
mod jsonl;
mod model;
#[cfg(test)]
mod tests;
mod tree;
mod writer;

pub use error::TraceError;
pub use header::TraceHeader;
pub use model::Trace;
pub use tree::SurfaceTree;
pub use writer::JsonlWriter;
