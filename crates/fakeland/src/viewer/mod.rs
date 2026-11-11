//! The nested viewer: a window on the host desktop per output that shows the composited output
//! and forwards pointer and keyboard input into the session. The compositor thread composites an
//! output into GBM dmabufs when the window asks for a picture, and the viewer thread attaches the
//! buffers to host surfaces without a copy. A host without dmabuf import gets CPU pixels through
//! shared memory. Viewer input enters the trace with the viewer as source.

mod client;
mod compose;
mod composer;
mod fps;
mod input;
mod layout;
mod link;
mod mailbox;
mod messages;
mod title;

pub use link::ViewerLink;
pub use messages::{AxisKind, FromViewer, OutputView, ToViewer, ViewerFrame, ViewerInput};
