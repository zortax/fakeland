//! The writer side of a recording: a bounded frame queue from the compositor thread, a writer
//! thread, and one sink per format. The compositor thread never waits for the writer: when the
//! queue is full, the frames become repeats of the last frame that reached the writer.

mod ffmpeg;
mod png_dir;
mod queue;
mod sink;
mod thread;
mod y4m_file;

pub use queue::{FrameQueue, Push};
pub use sink::Sink;
pub use thread::{WriterOutcome, WriterThread, open_writer};
