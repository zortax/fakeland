//! The active frame captures of a session.

use std::collections::HashMap;

use fakeland_types::{Nanos, WindowId};

use super::{CaptureSpec, CapturedFrame};
use crate::frames::CommitHookId;
use crate::server::State;

/// The handle of a frame capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameCaptureId(pub u64);

/// A callback that takes frames instead of the capture keeping them.
pub type FrameSink = Box<dyn FnMut(&mut State, CapturedFrame)>;

/// One active capture.
pub struct ActiveCapture {
    /// The spec.
    pub spec: CaptureSpec,
    /// The window it follows.
    pub window: WindowId,
    /// The frames so far.
    pub frames: Vec<CapturedFrame>,
    /// The count of frames taken, kept or sent to the sink.
    pub taken: usize,
    /// Frames past the limit or that failed.
    pub dropped: u64,
    /// The capture cost so far.
    pub cost: Nanos,
    /// The commit hook of commit captures.
    pub hook: Option<CommitHookId>,
    /// Takes frames instead of `frames`.
    pub sink: Option<FrameSink>,
}

/// Every active capture.
#[derive(Default)]
pub struct FrameCaptures {
    /// Captures by id.
    pub active: HashMap<FrameCaptureId, ActiveCapture>,
    next: u64,
}

impl std::fmt::Debug for FrameCaptures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameCaptures")
            .field("active", &self.active.len())
            .finish()
    }
}

impl FrameCaptures {
    /// Returns a new capture id.
    pub fn next_id(&mut self) -> FrameCaptureId {
        self.next += 1;
        FrameCaptureId(self.next)
    }
}
