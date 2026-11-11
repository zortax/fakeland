//! The frame record: one surface commit through every stage of the pipeline.

use fakeland_types::{
    CommitId, InputEventId, Nanos, OutputId, Physical, Rect, SurfaceId, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::event::{BufferInfo, SyncKind};

/// The latch of a commit for a vblank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatchInfo {
    /// The latch time.
    pub t: Timestamp,
    /// The output.
    pub output: OutputId,
    /// The vblank sequence number.
    pub seq: u64,
}

/// The presentation of a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentInfo {
    /// The time the compositor records the presentation.
    pub t: Timestamp,
    /// The presentation time that the feedback reports.
    pub time: Timestamp,
    /// The output.
    pub output: OutputId,
    /// The vblank sequence number.
    pub seq: u64,
    /// The refresh period.
    pub refresh: Option<Nanos>,
    /// The `wp_presentation_feedback` kind flags.
    pub flags: u32,
    /// The commit without content that carries the presentation. A latch can take this commit
    /// together with a later commit without content, and only the later one presents while the
    /// refresh shows the buffer of this commit. `None` when this commit presents itself.
    #[serde(default)]
    pub via: Option<CommitId>,
}

/// An app span mark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanMark {
    /// The phase name.
    pub name: String,
    /// The start of the phase.
    pub begin: Timestamp,
    /// The end of the phase.
    pub end: Timestamp,
}

impl SpanMark {
    /// Returns the length of the phase.
    pub fn duration(&self) -> Nanos {
        self.end - self.begin
    }
}

/// The app marks that a commit carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameMarks {
    /// The app frame id.
    pub frame_id: Option<u64>,
    /// The input events whose effect the commit shows.
    pub inputs_handled: Vec<InputEventId>,
    /// The phases of the client work.
    pub spans: Vec<SpanMark>,
    /// The named points.
    pub names: Vec<String>,
}

impl FrameMarks {
    /// Returns true when the commit carries no mark.
    pub fn is_empty(&self) -> bool {
        self.frame_id.is_none()
            && self.inputs_handled.is_empty()
            && self.spans.is_empty()
            && self.names.is_empty()
    }
}

/// The fate of a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameOutcome {
    /// The commit shows on an output.
    Presented,
    /// A newer commit replaces the commit before it shows.
    Discarded,
    /// The trace ends before the commit shows or drops.
    Pending,
}

impl FrameOutcome {
    /// Returns the snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Presented => "presented",
            Self::Discarded => "discarded",
            Self::Pending => "pending",
        }
    }
}

/// One surface commit with every stage and the stage times derived from them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRecord {
    /// The commit.
    pub commit: CommitId,
    /// The surface.
    pub surface: SurfaceId,
    /// The send time of the first frame callback after the previous commit of the surface.
    pub callback_sent: Option<Timestamp>,
    /// The commit time.
    pub committed: Timestamp,
    /// The time the fences signal. `None` when no `Ready` record exists.
    pub ready: Option<Timestamp>,
    /// The latch.
    pub latched: Option<LatchInfo>,
    /// The presentation. A content commit that a later commit without content replaced at the
    /// latch takes the presentation of that refresh, since the refresh shows its buffer.
    pub presented: Option<PresentInfo>,
    /// The time a newer commit replaces this one in the scheduler.
    pub discarded: Option<Timestamp>,
    /// The buffer release time.
    pub released: Option<Timestamp>,
    /// The newly attached buffer.
    pub buffer: Option<BufferInfo>,
    /// True when the commit attaches a null buffer.
    pub detached: bool,
    /// The buffer damage.
    pub damage: Vec<Rect<Physical>>,
    /// The damaged area in buffer pixels, overlaps counted once, clipped to the buffer.
    pub damage_area: i64,
    /// The serial of the last acknowledged configure.
    pub configure_serial: Option<u32>,
    /// The GPU synchronization.
    pub sync: SyncKind,
    /// True when the commit requests a frame callback.
    pub frame_callback: bool,
    /// True when the commit requests presentation feedback.
    pub presentation_feedback: bool,
    /// True when the commit sets a fifo barrier.
    pub fifo_barrier: bool,
    /// True when the commit waits for a fifo barrier.
    pub fifo_wait: bool,
    /// The commit-timing target.
    pub target: Option<Timestamp>,
    /// The app marks.
    pub marks: FrameMarks,
    /// The input events sent to the surface since its previous commit.
    pub inputs: Vec<InputEventId>,
    /// Callback to commit. `None` without a preceding callback.
    pub client_time: Option<Nanos>,
    /// Commit to ready. `None` for a commit without fences or without a `Ready` record.
    pub gpu_time: Option<Nanos>,
    /// Ready to latch. The commit time stands in for a missing ready time.
    pub queue_time: Option<Nanos>,
    /// Commit to presentation.
    pub display_latency: Option<Nanos>,
    /// Presentation time since the previous presented frame of the surface.
    pub present_interval: Option<Nanos>,
    /// Vblanks between this and the previous presented frame of the surface on the same output.
    pub vblanks_missed: Option<u64>,
}

impl FrameRecord {
    /// Returns the fate of the commit. A commit whose buffer shows counts as presented, also when
    /// a later commit without content replaced it at the latch.
    pub const fn outcome(&self) -> FrameOutcome {
        if self.presented.is_some() {
            FrameOutcome::Presented
        } else if self.discarded.is_some() {
            FrameOutcome::Discarded
        } else {
            FrameOutcome::Pending
        }
    }

    /// Returns the ready time, with the commit time for a commit without fences.
    pub fn effective_ready(&self) -> Timestamp {
        self.ready.unwrap_or(self.committed)
    }

    /// Returns true when the commit attaches a new buffer.
    pub const fn has_buffer(&self) -> bool {
        self.buffer.is_some()
    }

    /// Returns true when the commit changes the content: it attaches a new buffer or detaches the
    /// buffer. A commit without content only carries state, callbacks or barriers.
    pub const fn has_content(&self) -> bool {
        self.buffer.is_some() || self.detached
    }

    /// Returns true when the commit is a presented frame: a refresh shows its new content.
    /// A presented commit without content shows no new buffer and is no presented frame.
    pub const fn is_presented_frame(&self) -> bool {
        self.presented.is_some() && self.has_content()
    }

    /// Returns the presentation time.
    pub fn presented_time(&self) -> Option<Timestamp> {
        self.presented.map(|p| p.time)
    }

    /// Returns the length of the app span with a name.
    pub fn span(&self, name: &str) -> Option<Nanos> {
        self.marks
            .spans
            .iter()
            .find(|s| s.name == name)
            .map(SpanMark::duration)
    }
}
