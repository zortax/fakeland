//! The drag follow report.

use fakeland_trace::Summary;
use fakeland_types::{CommitId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The content position of one presented frame against the pointer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FollowSample {
    /// The commit.
    pub commit: CommitId,
    /// The presentation time.
    pub presented: Timestamp,
    /// The pointer displacement along the drag at the presentation.
    pub pointer_px: f64,
    /// The content displacement along the drag.
    pub content_px: f64,
    /// How far the content trails the pointer.
    pub lag_px: f64,
    /// How long ago the pointer was where the content is.
    pub lag_ms: Option<f64>,
}

/// How dragged content follows the pointer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragFollowReport {
    /// The captured frames.
    pub frames: usize,
    /// The pointer motions of the drag.
    pub motions: usize,
    /// The drag distance in logical pixels.
    pub distance_px: f64,
    /// Frame pairs that motion estimation rejected.
    pub rejected_pairs: usize,
    /// The pixel lag of frames presented during the drag.
    pub lag_px: Option<Summary>,
    /// The time lag of frames presented during the drag.
    pub lag_ms: Option<Summary>,
    /// The content displacement of the last frame over the drag distance.
    pub follow_ratio: Option<f64>,
    /// The distance between the final pointer and the final content position.
    pub final_error_px: Option<f64>,
    /// The frames presented during the drag.
    pub samples: Vec<FollowSample>,
}

impl Metrics for DragFollowReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("lag_ms_p50", self.lag_ms.p50(), Unit::Ms);
        m.lower("lag_ms_p95", self.lag_ms.p95(), Unit::Ms);
        m.lower("lag_px_p50", self.lag_px.p50(), Unit::Px);
        m.lower("lag_px_max", self.lag_px.max(), Unit::Px);
        m.lower("final_error", self.final_error_px, Unit::Px);
        m.neutral("follow_ratio", self.follow_ratio, Unit::Ratio);
        m.count("rejected_pairs", self.rejected_pairs);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.samples.len())
    }
}
