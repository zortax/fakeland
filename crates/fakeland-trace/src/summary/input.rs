//! The summary of input events.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::frames::{Attribution, FrameTimeline};
use crate::stats::Summary;

/// Input counts and latencies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputSummary {
    /// The number of input events.
    pub count: usize,
    /// The number of input events per kind.
    pub by_kind: BTreeMap<String, usize>,
    /// The number of events that an app mark attributes.
    pub attributed_by_mark: usize,
    /// The number of events that the next-commit rule attributes.
    pub attributed_by_next_commit: usize,
    /// The number of events without a handling commit.
    pub unattributed: usize,
    /// Send to handling commit, in milliseconds.
    pub handling_ms: Option<Summary>,
    /// Send to presentation, in milliseconds.
    pub latency_ms: Option<Summary>,
}

impl InputSummary {
    /// Summarizes the inputs of a timeline. Latencies count only events with user intent.
    pub fn build(tl: &FrameTimeline) -> Self {
        let mut s = Self::default();
        let mut handling = Vec::new();
        let mut latency = Vec::new();
        for i in tl.inputs() {
            s.count += 1;
            *s.by_kind.entry(i.kind.as_str().to_string()).or_default() += 1;
            match i.attribution {
                Attribution::Mark => s.attributed_by_mark += 1,
                Attribution::NextCommit => s.attributed_by_next_commit += 1,
                Attribution::None => s.unattributed += 1,
            }
            if i.kind.is_actionable() {
                handling.extend(i.handling_latency().map(|n| n.as_millis_f64()));
                latency.extend(i.latency().map(|n| n.as_millis_f64()));
            }
        }
        s.handling_ms = Summary::of(&handling);
        s.latency_ms = Summary::of(&latency);
        s
    }
}
