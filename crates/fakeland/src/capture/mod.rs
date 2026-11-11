//! Frame capture: screenshots of outputs, windows and regions with a pixel-to-logical mapping,
//! the composite of `Composite::EveryFrame`, and captures that the GPU scales down. Each capture
//! records its cost in the trace. Stepped captures wait for the GPU on the event loop and copy
//! their rows in chunks clear of output wakes.

mod job;
mod render;
mod scaled;
mod screenshot;
mod service;
mod timing;

pub use job::{CaptureJobs, CaptureStart};
pub use screenshot::{CaptureError, CaptureOptions, Screenshot};
pub use timing::CaptureCost;
