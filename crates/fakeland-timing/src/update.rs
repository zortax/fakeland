//! Content updates that clients submit to an output scheduler.

use fakeland_types::{CommitId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

/// When the buffer of an update is ready to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// The buffer is ready at this time. A time before the submission means "ready at once".
    Ready(Timestamp),
    /// The buffer waits for a fence. `Scheduler::mark_ready` reports the signal.
    Pending,
}

/// The fifo-v1 flags of an update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FifoFlags {
    /// The update sets the surface barrier when it applies.
    pub set_barrier: bool,
    /// The update waits until the surface barrier clears.
    pub wait_barrier: bool,
}

/// One surface commit as the display model sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update {
    /// The committed surface.
    pub surface: SurfaceId,
    /// The commit.
    pub commit: CommitId,
    /// When the buffer is ready.
    pub readiness: Readiness,
    /// The commit attaches a new buffer.
    pub has_content: bool,
    /// The commit requests a frame callback.
    pub wants_callback: bool,
    /// The fifo-v1 flags.
    pub fifo: FifoFlags,
    /// The commit-timing target: the update presents no earlier than this time.
    pub target: Option<Timestamp>,
}

impl Update {
    /// Builds a content update that is ready at submission and requests a frame callback.
    pub fn new(surface: SurfaceId, commit: CommitId) -> Self {
        Self {
            surface,
            commit,
            readiness: Readiness::Ready(Timestamp::ZERO),
            has_content: true,
            wants_callback: true,
            fifo: FifoFlags::default(),
            target: None,
        }
    }

    /// Builds an update without a new buffer that only requests a frame callback.
    pub fn callback_only(surface: SurfaceId, commit: CommitId) -> Self {
        Self::new(surface, commit).content(false)
    }

    /// Sets the ready time of the buffer.
    pub fn ready_at(mut self, at: Timestamp) -> Self {
        self.readiness = Readiness::Ready(at);
        self
    }

    /// Marks the buffer as waiting for a fence.
    pub fn pending(mut self) -> Self {
        self.readiness = Readiness::Pending;
        self
    }

    /// Sets whether the commit attaches a new buffer.
    pub fn content(mut self, has_content: bool) -> Self {
        self.has_content = has_content;
        self
    }

    /// Sets whether the commit requests a frame callback.
    pub fn callback(mut self, wants_callback: bool) -> Self {
        self.wants_callback = wants_callback;
        self
    }

    /// Sets the fifo-v1 flags.
    pub fn fifo(mut self, set_barrier: bool, wait_barrier: bool) -> Self {
        self.fifo = FifoFlags {
            set_barrier,
            wait_barrier,
        };
        self
    }

    /// Sets the commit-timing target.
    pub fn target(mut self, at: Timestamp) -> Self {
        self.target = Some(at);
        self
    }
}
