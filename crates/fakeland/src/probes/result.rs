//! Probe results: one record per repeat with the stage times of the detected frame, and the
//! report of a whole probe.

use fakeland_types::{CommitId, InputEventId, Nanos, ProbeId, SurfaceId, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The latency stages of one probe repeat. Each stage is `None` when one of its ends is unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeStages {
    /// Input send to the commit of the detected frame.
    pub input_to_commit: Option<Nanos>,
    /// Commit to fence ready.
    pub commit_to_ready: Option<Nanos>,
    /// Fence ready to the latch.
    pub ready_to_latch: Option<Nanos>,
    /// Latch to the modeled presentation.
    pub latch_to_present: Option<Nanos>,
    /// Input send to the modeled presentation.
    pub total: Option<Nanos>,
}

impl ProbeStages {
    /// Computes the stages from the stage times.
    pub fn compute(
        input: Option<Timestamp>,
        committed: Option<Timestamp>,
        ready: Option<Timestamp>,
        latched: Option<Timestamp>,
        presented: Option<Timestamp>,
    ) -> Self {
        let span = |a: Option<Timestamp>, b: Option<Timestamp>| Some(b?.since(a?));
        Self {
            input_to_commit: span(input, committed),
            commit_to_ready: span(committed, ready),
            ready_to_latch: span(ready, latched),
            latch_to_present: span(latched, presented),
            total: span(input, presented),
        }
    }
}

/// One repeat of a probe.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeResult {
    /// The probe id of this repeat.
    pub probe: Option<ProbeId>,
    /// The repeat index, from 0.
    pub iteration: u32,
    /// The input ids the repeat injected.
    pub inputs: Vec<InputEventId>,
    /// The send time of the first injected event.
    pub input_sent: Option<Timestamp>,
    /// The vblank phase the repeat asked for.
    pub requested_phase: Option<Nanos>,
    /// The phase of the input send within its refresh period.
    pub phase_in_frame: Option<Nanos>,
    /// The target was quiet before the input.
    pub settled: bool,
    /// The first commit that the detector matched.
    pub detected_commit: Option<CommitId>,
    /// The surface of the detected commit.
    pub surface: Option<SurfaceId>,
    /// The time the detector matched.
    pub detected_at: Option<Timestamp>,
    /// The commit time of the detected commit.
    pub committed: Option<Timestamp>,
    /// The fence ready time; the commit time without a fence.
    pub ready: Option<Timestamp>,
    /// The latch time of the frame that showed the change: the latch deadline of its refresh on
    /// a fixed grid, else the time the scheduler took it.
    pub latched: Option<Timestamp>,
    /// The modeled presentation time of the frame that showed the change.
    pub presented: Option<Timestamp>,
    /// The commit that presented the change: the detected commit, or a later one when the
    /// detected commit was discarded.
    pub presented_commit: Option<CommitId>,
    /// The vblank sequence of the presentation.
    pub presented_seq: Option<u64>,
    /// The latency stages.
    pub stages: ProbeStages,
    /// Nothing matched before the timeout.
    pub timed_out: bool,
    /// The count of client buffer readbacks.
    pub readbacks: u32,
    /// The compositor time spent on readbacks.
    pub readback_cost: Nanos,
}

/// Every repeat of one probe.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeReport {
    /// The probe label.
    pub label: Option<String>,
    /// The detector kind.
    pub detector: String,
    /// The repeats in order.
    pub results: Vec<ProbeResult>,
}

impl ProbeReport {
    /// Returns the total latency of every detected and presented repeat.
    pub fn totals(&self) -> Vec<Nanos> {
        self.results.iter().filter_map(|r| r.stages.total).collect()
    }

    /// Returns the input-to-commit latency of every detected repeat.
    pub fn input_to_commit(&self) -> Vec<Nanos> {
        self.results
            .iter()
            .filter_map(|r| r.stages.input_to_commit)
            .collect()
    }

    /// Returns the count of repeats that timed out.
    pub fn timeouts(&self) -> usize {
        self.results.iter().filter(|r| r.timed_out).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_follow_the_times() {
        let t = |ms: u64| Some(Timestamp(ms * 1_000_000));
        let s = ProbeStages::compute(t(10), t(17), t(18), t(25), t(26));
        assert_eq!(s.input_to_commit, Some(Nanos::from_millis(7)));
        assert_eq!(s.commit_to_ready, Some(Nanos::from_millis(1)));
        assert_eq!(s.ready_to_latch, Some(Nanos::from_millis(7)));
        assert_eq!(s.latch_to_present, Some(Nanos::from_millis(1)));
        assert_eq!(s.total, Some(Nanos::from_millis(16)));
        let partial = ProbeStages::compute(t(10), t(17), None, None, None);
        assert_eq!(partial.input_to_commit, Some(Nanos::from_millis(7)));
        assert_eq!(partial.total, None);
        let report = ProbeReport {
            label: None,
            detector: "any_damage".into(),
            results: vec![
                ProbeResult {
                    stages: s,
                    ..ProbeResult::default()
                },
                ProbeResult {
                    timed_out: true,
                    ..ProbeResult::default()
                },
            ],
        };
        assert_eq!(report.totals(), vec![Nanos::from_millis(16)]);
        assert_eq!(report.timeouts(), 1);
    }
}
