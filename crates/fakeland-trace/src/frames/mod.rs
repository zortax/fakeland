//! Frame timelines. `FrameTimeline::build` reads a trace and returns one `FrameRecord` per surface
//! commit with every pipeline stage and the derived stage times, plus the surfaces, windows,
//! clients, processes, outputs, vblanks and input attribution of the trace. A presented frame is a
//! refresh that shows a new buffer of a surface.

mod attribution;
mod builder;
mod damage;
mod derive;
mod entities;
mod inputs;
mod presented;
mod record;
mod shown;
mod timeline;
mod vblanks;

#[cfg(test)]
mod tests;

pub use damage::union_area;
pub use entities::{ClientInfo, ProcessInfo, SurfaceInfo, WindowInfo};
pub use inputs::{Attribution, InputRecord};
pub use presented::PresentedFrame;
pub use record::{FrameMarks, FrameOutcome, FrameRecord, LatchInfo, PresentInfo, SpanMark};
pub use timeline::FrameTimeline;
pub use vblanks::VblankInfo;
