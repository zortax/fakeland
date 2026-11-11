//! Chrome Trace Event JSON export that ui.perfetto.dev and chrome://tracing open. Each client is a
//! process, each surface owns stage tracks, input events link to the commits that handle them and
//! the presentations that show them through flow arrows, and samples become counter tracks.

mod chrome;
mod compositor;
mod counters;
mod emitter;
mod error;
mod frames;
mod inputs;
mod meters;
mod tracks;
mod write;

#[cfg(test)]
mod tests;

pub use error::PerfettoError;
pub use write::write_chrome_trace;
