//! The frame pipeline: commit hook, fence readiness, latch barriers, buffer holding and surface
//! bookkeeping. It connects smithay commits to the display schedulers.

mod apply;
mod buffer_info;
mod collect;
mod hook;
mod hooks;
mod pending;
mod readiness;
mod surfaces;

pub use buffer_info::BufferIds;
pub use collect::{Collected, collect};
pub use hooks::{CommitEvent, CommitHookId, CommitHooks, CommitPhase};
pub use pending::{Frames, PendingCommit};
pub use surfaces::{ApplyEntry, SurfaceRecord, Surfaces};
