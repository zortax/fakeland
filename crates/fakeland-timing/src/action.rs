//! The outputs of an output scheduler: what the compositor does at each deadline.

use fakeland_types::{CommitId, Nanos, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::PresentFlags;

/// One frame callback: the callbacks that a commit requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameCallback {
    /// The surface.
    pub surface: SurfaceId,
    /// The commit that requested the callback.
    pub commit: CommitId,
}

/// A step that the compositor carries out, in the order the scheduler returns them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// A display refresh. In unthrottled mode each present instant is a refresh.
    Vblank {
        /// The refresh sequence number.
        seq: u64,
        /// The ideal refresh time.
        ideal: Timestamp,
        /// The time the scheduler processed the refresh.
        woke: Timestamp,
        /// The compositor missed this refresh: nothing latched and content repeats.
        dropped: bool,
    },
    /// An update will present at the refresh `seq`. A later `Latched` with a higher `seq` replaces
    /// it when that refresh drops.
    Latched {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
        /// The refresh that presents the update.
        seq: u64,
    },
    /// An update reached the screen.
    Presented {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
        /// The refresh sequence number.
        seq: u64,
        /// The presentation time.
        time: Timestamp,
        /// The refresh period that the feedback reports.
        refresh: Option<Nanos>,
        /// The feedback flags.
        flags: PresentFlags,
        /// The commit whose buffer this refresh shows first: the presented commit, or a content
        /// update that it replaced at the latch. `None` when the refresh shows no new buffer.
        content: Option<CommitId>,
    },
    /// A newer update replaced this update before it reached the screen.
    Discarded {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
    },
    /// The buffer that this commit attached is free again.
    Released {
        /// The surface.
        surface: SurfaceId,
        /// The commit that attached the buffer.
        commit: CommitId,
    },
    /// Frame callbacks are due.
    FrameCallbacks {
        /// The callback time.
        time: Timestamp,
        /// The callbacks, sorted by surface and commit.
        callbacks: Vec<FrameCallback>,
    },
    /// The fifo barrier of a surface cleared.
    BarrierCleared {
        /// The surface.
        surface: SurfaceId,
    },
}
