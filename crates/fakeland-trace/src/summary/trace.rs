//! The summary of a whole trace.

use fakeland_types::Timestamp;
use serde::{Deserialize, Serialize};

use crate::event::Environment;
use crate::frames::FrameTimeline;
use crate::summary::{InputSummary, OutputSummary, SurfaceSummary};
use crate::trace::{Trace, TraceError};

/// Header facts, per-output and per-surface summaries, and input latency of a trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceSummary {
    /// The trace format version.
    pub format_version: u32,
    /// The host environment.
    pub environment: Environment,
    /// The trace start.
    pub started: Timestamp,
    /// The trace length in milliseconds.
    pub duration_ms: f64,
    /// The number of records.
    pub records: usize,
    /// The number of records a bounded recorder dropped.
    pub dropped_records: u64,
    /// The number of stage records that name an unknown commit.
    pub orphans: usize,
    /// The outputs.
    pub outputs: Vec<OutputSummary>,
    /// The surfaces with at least one commit.
    pub surfaces: Vec<SurfaceSummary>,
    /// The input events.
    pub inputs: InputSummary,
}

impl TraceSummary {
    /// Summarizes a trace and its timeline.
    pub fn build(trace: &Trace, timeline: &FrameTimeline) -> Self {
        let mut outputs: Vec<_> = timeline.outputs().map(|o| o.output).collect();
        for o in timeline.vblank_outputs() {
            if !outputs.contains(&o) {
                outputs.push(o);
            }
        }
        outputs.sort();
        Self {
            format_version: trace.header.format_version,
            environment: trace.header.environment.clone(),
            started: trace.header.started,
            duration_ms: trace.duration().as_millis_f64(),
            records: trace.records.len(),
            dropped_records: trace.header.dropped,
            orphans: timeline.orphans(),
            outputs: outputs
                .into_iter()
                .map(|o| OutputSummary::build(timeline, o))
                .collect(),
            surfaces: timeline
                .committed_surfaces()
                .map(|s| SurfaceSummary::build(timeline, s))
                .collect(),
            inputs: InputSummary::build(timeline),
        }
    }

    /// Builds the timeline of a trace and summarizes both.
    pub fn of(trace: &Trace) -> Self {
        Self::build(trace, &FrameTimeline::build(trace))
    }

    /// Writes the summary as pretty JSON.
    pub fn write_json(&self, w: impl std::io::Write) -> Result<(), TraceError> {
        serde_json::to_writer_pretty(w, self).map_err(|source| TraceError::Json { line: 0, source })
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Nanos, SurfaceId};

    use super::*;
    use crate::event::InputKind;
    use crate::testing::{steady, vblank_time};

    #[test]
    fn summary_of_a_steady_flow() {
        let mut f = steady(61);
        f.input(
            vblank_time(3) + Nanos::from_millis(1),
            1,
            1,
            InputKind::PointerMotion,
        );
        let mut trace = f.build();
        trace.records.sort_by_key(|r| r.t);
        let s = TraceSummary::of(&trace);
        assert_eq!(s.surfaces.len(), 1);
        let surf = &s.surfaces[0];
        assert_eq!(surf.surface, SurfaceId(1));
        assert_eq!(surf.commits, 61);
        assert_eq!(surf.presented, 61);
        assert_eq!(surf.discarded, 0);
        assert_eq!(surf.app_id.as_deref(), Some("org.test.app"));
        assert!((surf.fps.unwrap_or_default() - 60.0).abs() < 1e-3);
        assert!((surf.gpu_time_ms.unwrap().mean - 2.0).abs() < 1e-9);
        assert_eq!(surf.sync.explicit, 61);
        assert_eq!(surf.formats, vec!["XR24".to_string()]);
        assert_eq!(surf.vblanks_missed, 0);
        assert_eq!(s.outputs.len(), 1);
        assert_eq!(s.outputs[0].vblanks, 62);
        assert!((s.outputs[0].wake_lateness_us.unwrap().mean - 20.0).abs() < 1e-9);
        assert_eq!(s.inputs.count, 1);
        assert_eq!(s.inputs.attributed_by_next_commit, 1);
        let lat = s.inputs.latency_ms.unwrap().mean;
        // Commit 4 at vblank 3 + 5 ms handles the input and shows at vblank 4.
        let expected = (vblank_time(4) - (vblank_time(3) + Nanos::from_millis(1))).as_millis_f64();
        assert!((lat - expected).abs() < 1e-9, "{lat} vs {expected}");
        let mut json = Vec::new();
        s.write_json(&mut json).unwrap();
        let back: TraceSummary = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn commits_without_content_are_no_frames() {
        let mut f = steady(10);
        for n in 1..10u64 {
            let c = 100 + n;
            f.empty_commit(vblank_time(n) + Nanos::from_millis(6), 1, c);
            f.discard(vblank_time(n + 1) - Nanos::from_micros(1500), 1, c);
        }
        let mut trace = f.build();
        trace.records.sort_by_key(|r| r.t);
        let surf = &TraceSummary::of(&trace).surfaces[0];
        assert_eq!(surf.commits, 19);
        assert_eq!(surf.buffers, 10);
        assert_eq!(surf.presented, 10);
        assert_eq!(surf.discarded, 0);
        assert_eq!(surf.pending, 0);
        assert_eq!(surf.sync.explicit, 10);
        assert_eq!(surf.sync.none, 0);
        assert!((surf.gpu_time_ms.unwrap().mean - 2.0).abs() < 1e-9);
    }
}
