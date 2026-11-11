//! Video recording of an output or a window. Frames come from presented frames, composited and
//! read back on the compositor thread with their cost in the trace, and follow the fake vblanks:
//! the video has one slot per refresh, or per slot of a chosen rate, and repeats a picture for
//! every slot without a new presentation. A writer thread encodes through ffmpeg, a PNG
//! directory or a Y4M file behind a bounded queue.

mod canvas;
mod error;
mod follow;
mod handle;
mod pacer;
mod rate;
mod registry;
mod service;
mod spec;
mod writer;
mod y4m;

pub use canvas::Canvas;
pub use error::RecordError;
pub use handle::Recording;
pub use pacer::FramePacer;
pub use rate::FrameRate;
pub use registry::{RecordingInfo, Recordings};
pub use spec::{RecordSpec, RecordTarget, RecordingId, RecordingReport, VideoFormat};
pub use writer::WriterOutcome;
