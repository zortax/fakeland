//! The summary of one surface.

use std::collections::BTreeMap;

use fakeland_types::{ClientId, Nanos, SurfaceId, WindowId};
use serde::{Deserialize, Serialize};

use crate::event::SyncKind;
use crate::frames::{FrameOutcome, FrameRecord, FrameTimeline};
use crate::stats::{FpsLows, Summary};

/// Commit counts by synchronization kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCounts {
    /// Commits without fences.
    pub none: usize,
    /// Commits with implicit fences.
    pub implicit: usize,
    /// Commits with explicit sync.
    pub explicit: usize,
}

/// Frame counts, rate and stage statistics of one surface. Spans are in milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSummary {
    /// The surface.
    pub surface: SurfaceId,
    /// The owner.
    pub client: Option<ClientId>,
    /// The role.
    pub role: Option<String>,
    /// The parent of a subsurface or popup.
    pub parent: Option<SurfaceId>,
    /// The window whose tree holds the surface.
    pub window: Option<WindowId>,
    /// The app id of the window.
    pub app_id: Option<String>,
    /// The number of commits.
    pub commits: usize,
    /// The number of commits with a new buffer.
    pub buffers: usize,
    /// The number of presented content commits.
    pub presented: usize,
    /// The number of discarded content commits.
    pub discarded: usize,
    /// The number of content commits without an end in the trace.
    pub pending: usize,
    /// Presented frames per second between the first and last presentation.
    pub fps: Option<f64>,
    /// Frame-rate lows of the present intervals.
    pub lows: Option<FpsLows>,
    /// Present intervals.
    pub present_interval_ms: Option<Summary>,
    /// Callback to commit.
    pub client_time_ms: Option<Summary>,
    /// Commit to ready, for commits with fences.
    pub gpu_time_ms: Option<Summary>,
    /// Ready to latch.
    pub queue_time_ms: Option<Summary>,
    /// Commit to presentation.
    pub display_latency_ms: Option<Summary>,
    /// The sum of missed vblanks between presentations.
    pub vblanks_missed: u64,
    /// The number of presented frames with at least one missed vblank before them.
    pub frames_after_miss: usize,
    /// The damaged area per commit in buffer pixels.
    pub damage_area: Option<Summary>,
    /// Commit counts by synchronization kind.
    pub sync: SyncCounts,
    /// The distinct buffer formats as fourcc strings.
    pub formats: Vec<String>,
    /// App span lengths by span name.
    pub app_spans_ms: BTreeMap<String, Summary>,
}

impl SurfaceSummary {
    /// Summarizes one surface of a timeline.
    pub fn build(tl: &FrameTimeline, surface: SurfaceId) -> Self {
        let frames = tl.frames(surface);
        let info = tl.surface(surface);
        let window = tl.window_of(surface);
        let content = || frames.iter().filter(|f| f.has_content());
        let count = |o: FrameOutcome| content().filter(|f| f.outcome() == o).count();
        let stage =
            |get: fn(&FrameRecord) -> Option<Nanos>| Summary::of_millis(content().filter_map(get));
        let presented = tl.presented_frames(surface, None);
        let fps = match (presented.first(), presented.last()) {
            (Some(a), Some(b)) if presented.len() > 1 && b.time > a.time => {
                Some((presented.len() - 1) as f64 / (b.time - a.time).as_secs_f64())
            }
            _ => None,
        };
        let mut sync = SyncCounts::default();
        let mut formats = Vec::new();
        let mut spans: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for f in content() {
            match f.sync {
                SyncKind::None => sync.none += 1,
                SyncKind::Implicit => sync.implicit += 1,
                SyncKind::Explicit => sync.explicit += 1,
            }
            if let Some(b) = &f.buffer {
                let cc = b.fourcc();
                if !formats.contains(&cc) {
                    formats.push(cc);
                }
            }
            for s in &f.marks.spans {
                spans
                    .entry(s.name.clone())
                    .or_default()
                    .push(s.duration().as_millis_f64());
            }
        }
        let damage: Vec<f64> = frames.iter().map(|f| f.damage_area as f64).collect();
        Self {
            surface,
            client: info.and_then(|s| s.client),
            role: info.and_then(|s| s.role.clone()),
            parent: info.and_then(|s| s.parent),
            window,
            app_id: window.and_then(|w| tl.window(w)).map(|w| w.app_id.clone()),
            commits: frames.len(),
            buffers: frames.iter().filter(|f| f.has_buffer()).count(),
            presented: count(FrameOutcome::Presented),
            discarded: count(FrameOutcome::Discarded),
            pending: count(FrameOutcome::Pending),
            fps,
            lows: FpsLows::from_intervals(frames.iter().filter_map(|f| f.present_interval)),
            present_interval_ms: stage(|f| f.present_interval),
            client_time_ms: stage(|f| f.client_time),
            gpu_time_ms: stage(|f| f.gpu_time),
            queue_time_ms: stage(|f| f.queue_time),
            display_latency_ms: stage(|f| f.display_latency),
            vblanks_missed: frames.iter().filter_map(|f| f.vblanks_missed).sum(),
            frames_after_miss: frames
                .iter()
                .filter(|f| f.vblanks_missed.is_some_and(|m| m > 0))
                .count(),
            damage_area: Summary::of(&damage),
            sync,
            formats,
            app_spans_ms: spans
                .into_iter()
                .filter_map(|(k, v)| Some((k, Summary::of(&v)?)))
                .collect(),
        }
    }
}
