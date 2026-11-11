//! Input events and the commits that handle them.

use fakeland_types::{CommitId, InputEventId, Nanos, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::event::{InputKind, InputSource};

/// How an input event maps to the commit that handles it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    /// An `input_handled` app mark names the commit.
    Mark,
    /// The commit is the first commit with a buffer in the surface tree after the event.
    NextCommit,
    /// No commit handles the event.
    None,
}

/// One input event with its handling commit and the presentation that shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputRecord {
    /// The input event.
    pub id: InputEventId,
    /// The send time, after the flush to the client socket.
    pub sent: Timestamp,
    /// The kind.
    pub kind: InputKind,
    /// The receiving surface.
    pub surface: Option<SurfaceId>,
    /// The global logical horizontal position.
    pub x: Option<f64>,
    /// The global logical vertical position.
    pub y: Option<f64>,
    /// The detail text.
    pub detail: String,
    /// The origin.
    pub source: InputSource,
    /// The commit that handles the event.
    pub handled_by: Option<CommitId>,
    /// How `handled_by` was found.
    pub attribution: Attribution,
    /// The commit time of the handling commit.
    pub handled_at: Option<Timestamp>,
    /// The commit whose presentation first shows the effect: the handling commit, or the next
    /// presented commit of its surface when the handling commit drops.
    pub presented_commit: Option<CommitId>,
    /// The presentation time of `presented_commit`.
    pub presented: Option<Timestamp>,
}

impl InputRecord {
    /// Returns the span from send to the handling commit.
    pub fn handling_latency(&self) -> Option<Nanos> {
        self.handled_at.map(|t| t - self.sent)
    }

    /// Returns the span from send to presentation.
    pub fn latency(&self) -> Option<Nanos> {
        self.presented.map(|t| t - self.sent)
    }
}
