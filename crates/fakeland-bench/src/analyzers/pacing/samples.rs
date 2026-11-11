//! The pacing samples of one surface, extracted from a timeline. Samples of several iterations
//! merge into one pooled report.

use fakeland_trace::frames::FrameOutcome;
use fakeland_types::{CommitId, Nanos, OutputId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::input::AnalysisInput;

/// One presented frame: a refresh that shows a new buffer of the surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacingFrame {
    /// The commit.
    pub commit: CommitId,
    /// The app frame id.
    pub frame_id: Option<u64>,
    /// The modeled presentation time.
    pub time: Timestamp,
    /// The time since the previous presented frame of the same iteration.
    pub interval: Option<Nanos>,
    /// The vblanks without a new frame before this one.
    pub vblanks_missed: Option<u64>,
    /// The commit-timing target.
    pub target: Option<Timestamp>,
}

/// The pacing samples of one or more iterations.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PacingSamples {
    /// The surface.
    pub surface: Option<SurfaceId>,
    /// The refresh period of the output.
    pub period: Option<Nanos>,
    /// The commit times of the surface.
    pub commits: Vec<Timestamp>,
    /// The count of discarded content commits whose buffer never shows.
    pub discarded: usize,
    /// The count of commits without an outcome.
    pub pending: usize,
    /// The count of commits with a fifo barrier or wait.
    pub fifo: usize,
    /// The count of presentations without the vsync flag.
    pub without_vsync: usize,
    /// The presented frames in presentation order.
    pub frames: Vec<PacingFrame>,
    /// The total time from first to last presentation of each iteration.
    pub span: Nanos,
    /// The total time from first to last commit of each iteration.
    pub commit_span: Nanos,
}

/// The vsync flag of `wp_presentation_feedback`.
const VSYNC: u32 = 1;

impl PacingSamples {
    /// Extracts the samples of a surface, or of the main surface of the input.
    pub fn extract(input: &AnalysisInput, surface: Option<SurfaceId>) -> Self {
        let Some(surface) = surface.or_else(|| input.main_surface()) else {
            return Self::default();
        };
        let tl = &input.timeline;
        let records: Vec<_> = tl
            .frames(surface)
            .iter()
            .filter(|f| f.has_content())
            .collect();
        let presented = tl.presented_frames(surface, None::<OutputId>);
        let frames: Vec<PacingFrame> = presented
            .iter()
            .map(|p| {
                let f = tl.frame(p.commit);
                PacingFrame {
                    commit: p.commit,
                    frame_id: f.and_then(|f| f.marks.frame_id),
                    time: p.time,
                    interval: p.interval,
                    vblanks_missed: p.vblanks_missed,
                    target: f.and_then(|f| f.target),
                }
            })
            .collect();
        let output = presented.first().map(|p| p.output);
        let span = match (frames.first(), frames.last()) {
            (Some(a), Some(b)) => b.time.since(a.time),
            _ => Nanos::ZERO,
        };
        let commits: Vec<Timestamp> = records.iter().map(|f| f.committed).collect();
        let commit_span = match (commits.first(), commits.last()) {
            (Some(a), Some(b)) => b.since(*a),
            _ => Nanos::ZERO,
        };
        Self {
            surface: Some(surface),
            period: input.period(output),
            discarded: records
                .iter()
                .filter(|f| f.outcome() == FrameOutcome::Discarded)
                .count(),
            pending: records
                .iter()
                .filter(|f| f.outcome() == FrameOutcome::Pending)
                .count(),
            fifo: records
                .iter()
                .filter(|f| f.fifo_barrier || f.fifo_wait)
                .count(),
            without_vsync: records
                .iter()
                .filter_map(|f| f.presented)
                .filter(|p| p.flags & VSYNC == 0)
                .count(),
            commits,
            frames,
            span,
            commit_span,
        }
    }

    /// Appends the samples of another iteration.
    pub fn merge(&mut self, other: Self) {
        self.surface = self.surface.or(other.surface);
        self.period = self.period.or(other.period);
        self.commits.extend(other.commits);
        self.discarded += other.discarded;
        self.pending += other.pending;
        self.fifo += other.fifo;
        self.without_vsync += other.without_vsync;
        self.frames.extend(other.frames);
        self.span += other.span;
        self.commit_span += other.commit_span;
    }

    /// Returns the present intervals.
    pub fn intervals(&self) -> Vec<Nanos> {
        self.frames.iter().filter_map(|f| f.interval).collect()
    }
}
