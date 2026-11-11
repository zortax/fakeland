//! The analyzer of ping-pong samples.

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use super::PingPongSample;
use crate::analyzer::Analyzer;
use crate::analyzers::util::SampleStats;
use crate::input::AnalysisInput;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// Summarizes the [`PingPongSample`]s that a ping-pong reactor emitted.
#[derive(Clone, Debug, Default)]
pub struct PingPongLatency;

/// The latency of ping-pong rounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PingPongReport {
    /// The rounds.
    pub rounds: usize,
    /// Send to the answering commit.
    pub input_to_commit_ms: Option<Summary>,
    /// Send to the modeled presentation of the answer.
    pub total_ms: Option<Summary>,
    /// The rounds per second, which a faster pipeline raises.
    pub rounds_per_s: Option<f64>,
}

impl Metrics for PingPongReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("total_p50", self.total_ms.p50(), Unit::Ms);
        m.lower("total_p95", self.total_ms.p95(), Unit::Ms);
        m.lower(
            "input_to_commit_p50",
            self.input_to_commit_ms.p50(),
            Unit::Ms,
        );
        m.higher("rounds_per_s", self.rounds_per_s, Unit::Count);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.rounds)
    }
}

impl Analyzer for PingPongLatency {
    type Report = PingPongReport;

    fn name(&self) -> String {
        "ping_pong".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> PingPongReport {
        let samples: Vec<PingPongSample> = input
            .values
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        let ms = |f: fn(&PingPongSample) -> f64| -> Vec<f64> { samples.iter().map(f).collect() };
        let span = match (samples.first(), samples.last()) {
            (Some(a), Some(b)) if samples.len() > 1 => {
                Some(b.presented.since(a.sent).as_secs_f64())
            }
            _ => None,
        };
        PingPongReport {
            rounds: samples.len(),
            input_to_commit_ms: Summary::of(&ms(|s| s.committed.since(s.sent).as_millis_f64())),
            total_ms: Summary::of(&ms(|s| s.presented.since(s.sent).as_millis_f64())),
            rounds_per_s: span.filter(|s| *s > 0.0).map(|s| samples.len() as f64 / s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Synth, at_ms};
    use fakeland_types::{CommitId, InputEventId};

    #[test]
    fn summarizes_samples() {
        let values = (0..10u32)
            .map(|i| {
                let t = f64::from(i) * 20.0;
                serde_json::to_value(PingPongSample {
                    round: i,
                    input: InputEventId(u64::from(i)),
                    sent: at_ms(t),
                    commit: CommitId(u64::from(i)),
                    committed: at_ms(t + 5.0),
                    presented: at_ms(t + 15.0),
                })
                .unwrap()
            })
            .collect();
        let r = PingPongLatency.analyze(&Synth::new().analysis().with_values(values));
        assert_eq!(r.rounds, 10);
        assert!((r.total_ms.unwrap().p50 - 15.0).abs() < 1e-6);
        assert!((r.input_to_commit_ms.unwrap().p50 - 5.0).abs() < 1e-6);
        assert!((r.rounds_per_s.unwrap() - 10.0 / 0.195).abs() < 1e-6);
    }
}
