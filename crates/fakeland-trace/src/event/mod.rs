//! Trace events. A `Record` pairs a compositor timestamp with one `Event`. Event variants are
//! append-only; a breaking change bumps `TRACE_FORMAT_VERSION`.

mod buffer;
mod environment;
mod input;
mod kind;
mod mark;
mod output;
mod query;
mod record;
mod version;

pub use buffer::{BufferInfo, BufferKind, SyncKind};
pub use environment::Environment;
pub use input::{InputKind, InputSource};
pub use kind::Event;
pub use mark::AppMarkKind;
pub use output::OutputInfo;
pub use query::ContextKey;
pub use record::Record;
pub use version::TRACE_FORMAT_VERSION;
