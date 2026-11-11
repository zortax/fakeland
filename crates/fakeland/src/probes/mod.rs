//! Latency probes. A probe injects an input at a chosen vblank phase and finds the first commit
//! of its target that shows the response: damage in a region, changed or matching pixels in a
//! region read back from the client buffer, or an app mark. The result carries the stage times
//! of that commit up to its modeled presentation.

mod detect;
mod geometry;
mod guard;
mod hit;
mod prepare;
mod queue;
mod readback;
mod result;
mod run;
mod watch;

pub use result::{ProbeReport, ProbeResult, ProbeStages};
