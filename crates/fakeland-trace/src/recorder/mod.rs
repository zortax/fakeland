//! The recorder: an append-only record buffer owned by the compositor thread, with a bounded ring
//! mode for streaming and capture spans that cut traces out of the buffer.

mod buffer;
mod capture;
mod error;
mod outputs;
#[cfg(test)]
mod tests;

pub use buffer::{Recorder, RecorderMode};
pub use capture::CaptureId;
pub use error::RecorderError;
