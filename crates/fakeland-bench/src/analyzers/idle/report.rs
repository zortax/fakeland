//! The idle efficiency report.

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// Damage against the real change between consecutive captured commits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DamageStats {
    /// The commit pairs compared.
    pub frames: usize,
    /// Damaged area over changed area per frame; 1 means exact.
    pub over_damage_ratio: Option<Summary>,
    /// The fraction of the damaged area that did not change.
    pub unchanged_damage_fraction: Option<Summary>,
    /// Changed pixels outside the damage, summed: a client bug.
    pub under_damage_pixels: u64,
    /// Frames with changed pixels outside the damage.
    pub under_damaged_frames: usize,
}

/// Activity while the target was suspended.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SuspendStats {
    /// The suspended time.
    pub duration_ms: f64,
    /// Commits with a buffer while suspended.
    pub commits: usize,
    /// Commits that asked for a frame callback while suspended.
    pub callbacks_requested: usize,
    /// Frame callbacks the compositor sent while suspended.
    pub callbacks_sent: usize,
}

/// How much a client does while nothing changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdleReport {
    /// The analyzed span.
    pub duration_ms: f64,
    /// Every commit of the target tree.
    pub commits: usize,
    /// Commits with a new buffer.
    pub buffer_commits: usize,
    /// Buffer commits per second.
    pub commits_per_s: f64,
    /// Buffer commits with no damage.
    pub empty_damage_commits: usize,
    /// Captured commits whose pixels equal the previous capture; `None` without a capture.
    pub unchanged_commits: Option<usize>,
    /// Buffer commits that changed nothing: unchanged captures, else empty damage.
    pub wasted_commits: usize,
    /// Wasted over buffer commits.
    pub wasted_fraction: f64,
    /// Commits that asked for a frame callback.
    pub callbacks_requested: usize,
    /// Frame callback requests per second.
    pub callbacks_per_s: f64,
    /// Damage accuracy, with a capture of every commit.
    pub damage: Option<DamageStats>,
    /// Activity while suspended.
    pub suspended: SuspendStats,
}

impl Metrics for IdleReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("commits_per_s", Some(self.commits_per_s), Unit::Count);
        m.count("wasted_commits", self.wasted_commits);
        m.lower(
            "wasted_fraction",
            Some(self.wasted_fraction),
            Unit::Fraction,
        );
        m.lower("callbacks_per_s", Some(self.callbacks_per_s), Unit::Count);
        if let Some(d) = &self.damage {
            m.lower("over_damage_p50", d.over_damage_ratio.p50(), Unit::Ratio);
            m.lower(
                "unchanged_damage_p50",
                d.unchanged_damage_fraction.p50(),
                Unit::Fraction,
            );
            m.lower(
                "under_damage_pixels",
                Some(d.under_damage_pixels as f64),
                Unit::Count,
            );
        }
        m.count("suspended_commits", self.suspended.commits);
        m.count("suspended_callbacks_sent", self.suspended.callbacks_sent);
        m.into_vec()
    }
}
