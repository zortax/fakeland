//! Presented frames: the frames that reach an output, in presentation order.

use fakeland_types::{CommitId, Nanos, OutputId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

/// One presented frame of a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentedFrame {
    /// The commit.
    pub commit: CommitId,
    /// The surface.
    pub surface: SurfaceId,
    /// The output.
    pub output: OutputId,
    /// The vblank sequence number.
    pub seq: u64,
    /// The presentation time.
    pub time: Timestamp,
    /// The commit time.
    pub committed: Timestamp,
    /// Presentation time since the previous presented frame of the surface in the sequence.
    pub interval: Option<Nanos>,
    /// Vblanks between this and the previous presented frame of the surface in the sequence.
    pub vblanks_missed: Option<u64>,
}
