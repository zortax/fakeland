//! Per-commit frame capture for analyzers. A capture follows one window and keeps an image of a
//! region for every applied commit or every presented frame, keyed by commit, up to a frame
//! limit.

mod registry;
mod spec;
mod stream;

pub use registry::{ActiveCapture, FrameCaptureId, FrameCaptures, FrameSink};
pub use spec::{CaptureSpec, CapturedFrame, FrameCapture};
