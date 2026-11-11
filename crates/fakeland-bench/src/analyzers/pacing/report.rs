//! The frame pacing report.

use fakeland_trace::Summary;
use fakeland_trace::stats::FpsLows;
use fakeland_types::{CommitId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use super::PresentMode;
use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// A frame whose present interval broke the jank rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JankFrame {
    /// The commit.
    pub commit: CommitId,
    /// The app frame id.
    pub frame_id: Option<u64>,
    /// The presentation time.
    pub presented: Timestamp,
    /// The present interval that ended with this frame.
    pub interval_ms: f64,
    /// The vblanks without a new frame before this one.
    pub vblanks_missed: Option<u64>,
}

/// Jank: present intervals longer than `factor` times the median interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JankReport {
    /// The factor of the rule.
    pub factor: f64,
    /// The median present interval.
    pub median_interval_ms: f64,
    /// The interval above which a frame is jank.
    pub threshold_ms: f64,
    /// The count of jank frames.
    pub count: usize,
    /// Jank frames over present intervals.
    pub fraction: f64,
    /// The jank frames in presentation order.
    pub frames: Vec<JankFrame>,
}

/// Commit and present rates against the refresh rate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadenceReport {
    /// Commits per second over the commit span.
    pub commit_rate_hz: Option<f64>,
    /// Presented frames per second over the present span.
    pub present_rate_hz: Option<f64>,
    /// The nominal refresh rate.
    pub refresh_hz: Option<f64>,
    /// Commits per refresh: 1 for a client that renders every refresh, above 1 for a client that
    /// renders faster than the display.
    pub commits_per_refresh: Option<f64>,
    /// Presented frames per refresh: 1 when every refresh shows a new frame.
    pub presents_per_refresh: Option<f64>,
}

/// How presentations follow commit-timing targets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitTimingReport {
    /// The presented frames with a target.
    pub frames: usize,
    /// Frames presented a refresh or more after their target.
    pub late: usize,
    /// Frames presented before their target, which breaks the protocol.
    pub early: usize,
    /// Presentation minus target in milliseconds.
    pub error_ms: Option<Summary>,
}

/// Frames presented in one second.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpsBucket {
    /// The second since the first presentation, from 0.
    pub second: u32,
    /// The frames presented in that second.
    pub frames: usize,
}

/// The frame pacing of one surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FramePacingReport {
    /// The surface.
    pub surface: Option<SurfaceId>,
    /// The nominal refresh rate.
    pub refresh_hz: Option<f64>,
    /// The commits of the surface.
    pub commits: usize,
    /// The presented frames.
    pub presented: usize,
    /// The discarded commits.
    pub discarded: usize,
    /// Commits without an outcome when the trace ended.
    pub pending: usize,
    /// The time from the first to the last presentation.
    pub duration_ms: f64,
    /// Presented frames per second: intervals over the present span.
    pub fps: Option<f64>,
    /// The present intervals.
    pub interval_ms: Option<Summary>,
    /// The standard deviation of the present intervals.
    pub jitter_ms: Option<f64>,
    /// The mean distance of an interval from the median interval.
    pub pacing_error_ms: Option<f64>,
    /// Average and low frame rates.
    pub lows: Option<FpsLows>,
    /// Vblanks that showed an old frame while the client kept presenting.
    pub missed_vblanks: u64,
    /// Frames that stayed on screen longer than one refresh.
    pub repeated_frames: usize,
    /// Long intervals.
    pub jank: JankReport,
    /// Rates against the refresh.
    pub cadence: CadenceReport,
    /// Discarded commits over all commits.
    pub discard_fraction: f64,
    /// The present mode the frames suggest.
    pub mode: PresentMode,
    /// Commit-timing adherence, when frames carry targets.
    pub commit_timing: Option<CommitTimingReport>,
    /// Frames per whole second since the first presentation.
    pub fps_series: Vec<FpsBucket>,
}

impl Metrics for FramePacingReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.higher("fps", self.fps, Unit::Fps);
        m.neutral("interval_p50", self.interval_ms.p50(), Unit::Ms);
        m.lower("interval_p99", self.interval_ms.p99(), Unit::Ms);
        m.lower("interval_max", self.interval_ms.max(), Unit::Ms);
        m.lower("jitter", self.jitter_ms, Unit::Ms);
        m.lower("pacing_error", self.pacing_error_ms, Unit::Ms);
        m.higher("low_1pct_fps", self.lows.map(|l| l.low_1pct_fps), Unit::Fps);
        m.higher(
            "low_01pct_fps",
            self.lows.map(|l| l.low_01pct_fps),
            Unit::Fps,
        );
        m.count("jank_count", self.jank.count);
        m.lower("jank_fraction", Some(self.jank.fraction), Unit::Fraction);
        m.lower(
            "missed_vblanks",
            Some(self.missed_vblanks as f64),
            Unit::Count,
        );
        m.neutral("discarded", Some(self.discarded as f64), Unit::Count);
        m.neutral(
            "commits_per_refresh",
            self.cadence.commits_per_refresh,
            Unit::Ratio,
        );
        m.higher(
            "presents_per_refresh",
            self.cadence.presents_per_refresh,
            Unit::Ratio,
        );
        if let Some(ct) = &self.commit_timing {
            m.count("commit_timing_late", ct.late);
            m.lower("commit_timing_error_p50", ct.error_ms.p50(), Unit::Ms);
        }
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.interval_ms.n())
    }
}
