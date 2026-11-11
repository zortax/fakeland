//! The buffers report.

use std::collections::BTreeMap;

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The commits of one format and modifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatCount {
    /// The fourcc, such as `XR24`.
    pub fourcc: String,
    /// The modifier.
    pub modifier: Option<u64>,
    /// The commit count.
    pub count: usize,
}

/// How the resolution of presented buffers fits the output scale.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleFit {
    /// Buffers with the pixel size their logical size needs, within one pixel.
    pub native: usize,
    /// Buffers with fewer pixels, which the compositor stretches.
    pub upscaled: usize,
    /// Buffers with more pixels, which cost memory and fill rate.
    pub oversampled: usize,
}

/// The buffer use of the target tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuffersReport {
    /// Commits with a buffer.
    pub buffer_commits: usize,
    /// Distinct buffers.
    pub distinct_buffers: usize,
    /// Distinct buffer sizes.
    pub distinct_sizes: usize,
    /// Buffers attached and not released, sampled at each buffer commit.
    pub in_flight: Option<Summary>,
    /// The largest count in flight.
    pub max_in_flight: usize,
    /// Buffers first seen more than a second after the first buffer commit.
    pub late_allocations: usize,
    /// Late allocations per second after the first second.
    pub churn_per_s: Option<f64>,
    /// Commits per buffer kind.
    pub kinds: BTreeMap<String, usize>,
    /// Commits per format and modifier.
    pub formats: Vec<FormatCount>,
    /// Commits per sync kind.
    pub sync: BTreeMap<String, usize>,
    /// The scale fit of presented buffers.
    pub scale: ScaleFit,
}

impl Metrics for BuffersReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.neutral(
            "distinct_buffers",
            Some(self.distinct_buffers as f64),
            Unit::Count,
        );
        m.neutral("in_flight_mean", self.in_flight.mean(), Unit::Count);
        m.neutral(
            "max_in_flight",
            Some(self.max_in_flight as f64),
            Unit::Count,
        );
        m.lower("churn_per_s", self.churn_per_s, Unit::Count);
        m.count("upscaled_frames", self.scale.upscaled);
        m.count("oversampled_frames", self.scale.oversampled);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        (self.buffer_commits > 0).then_some(self.buffer_commits)
    }
}
