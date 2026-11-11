//! The sink interface of the writer thread. A sink takes canvas-sized frames, each with a repeat
//! count, so repeats cost no conversion.

use fakeland_vision::ImageView;

/// One output format.
pub trait Sink: Send {
    /// Writes a frame `count` times.
    fn write(&mut self, frame: &ImageView<'_>, count: u64) -> Result<(), String>;

    /// Flushes and closes the output.
    fn finish(self: Box<Self>) -> Result<(), String>;
}
