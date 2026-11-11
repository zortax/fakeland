//! Per-surface display state: the ordered queue of content updates, the update latched for the
//! next refresh, the buffer on screen and the fifo barrier.

mod entry;
mod latch;
mod present;
mod state;

pub(crate) use entry::Entry;
pub(crate) use present::Frame;
pub(crate) use state::{SurfaceState, TargetLimit};
