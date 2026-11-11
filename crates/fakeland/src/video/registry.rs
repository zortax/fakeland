//! The active recordings of a session and the plan of a recording before it starts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use fakeland_types::{Nanos, OutputId, Timestamp, WindowId};
use fakeland_vision::Image;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::writer::{FrameQueue, WriterThread};
use super::{Canvas, FramePacer, FrameRate, RecordSpec, RecordingId, VideoFormat};
use crate::waits::ObserverId;

/// The resolved target of a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// An output.
    Output(OutputId),
    /// A window.
    Window(WindowId),
}

/// A recording that is ready to start: the target, size and rate are fixed.
#[derive(Clone, Debug)]
pub struct RecordPlan {
    /// The request.
    pub spec: RecordSpec,
    /// The format.
    pub format: VideoFormat,
    /// The target.
    pub source: Source,
    /// The canvas.
    pub canvas: Canvas,
    /// The frame rate.
    pub rate: FrameRate,
    /// The capture scale relative to the output scale.
    pub factor: f64,
}

/// One running recording.
#[derive(Debug)]
pub struct ActiveRecording {
    /// The plan.
    pub plan: RecordPlan,
    /// The output whose vblanks pace the video.
    pub output: OutputId,
    /// The slot clock.
    pub pacer: FramePacer,
    /// The latest composite.
    pub current: Arc<Image>,
    /// The latest composite has not gone to the writer yet.
    pub fresh: bool,
    /// The screen changed since the latest composite.
    pub dirty: bool,
    /// A composite task is queued.
    pub scheduled: bool,
    /// The frame queue.
    pub queue: FrameQueue,
    /// The writer thread.
    pub writer: WriterThread,
    /// The record observer.
    pub observer: ObserverId,
    /// Slots with a new composite.
    pub rendered: u64,
    /// Slots that repeat a composite.
    pub repeated: u64,
    /// The time of every composite.
    pub cost: Nanos,
}

/// A running recording as the session API lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordingInfo {
    /// The recording.
    pub id: RecordingId,
    /// The output file or directory.
    pub path: PathBuf,
    /// The format.
    pub format: VideoFormat,
    /// The frame width.
    pub width: u32,
    /// The frame height.
    pub height: u32,
    /// The frame rate.
    pub rate: FrameRate,
    /// The time of the first slot.
    pub first_slot: Timestamp,
    /// Slots so far.
    pub frames: u64,
    /// Slots that repeat a stale frame because the writer fell behind.
    pub dropped: u64,
}

impl ActiveRecording {
    /// Returns the listing of the recording.
    pub fn info(&self, id: RecordingId) -> RecordingInfo {
        RecordingInfo {
            id,
            path: self.plan.spec.path.clone(),
            format: self.plan.format,
            width: self.plan.canvas.width,
            height: self.plan.canvas.height,
            rate: self.plan.rate,
            first_slot: self.pacer.origin(),
            frames: self.pacer.emitted(),
            dropped: self.queue.dropped(),
        }
    }
}

/// Every running recording.
#[derive(Debug, Default)]
pub struct Recordings {
    /// Recordings by id.
    pub active: HashMap<RecordingId, ActiveRecording>,
    next: u64,
}

impl Recordings {
    /// Returns a new id.
    pub fn next_id(&mut self) -> RecordingId {
        self.next += 1;
        RecordingId(self.next)
    }
}
