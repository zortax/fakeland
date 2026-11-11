//! The throughput analyzer.

use fakeland_trace::SyncKind;
use fakeland_trace::frames::FrameOutcome;
use fakeland_types::Nanos;

use super::ThroughputReport;
use crate::analyzer::Analyzer;
use crate::analyzers::util::summary_ms;
use crate::input::AnalysisInput;

/// Measures the frame rate and GPU time of the target's main surface.
#[derive(Clone, Debug, Default)]
pub struct Throughput;

impl Analyzer for Throughput {
    type Report = ThroughputReport;

    fn name(&self) -> String {
        "throughput".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> ThroughputReport {
        let frames: Vec<_> = input
            .main_surface()
            .map(|s| input.timeline.frames(s))
            .unwrap_or_default()
            .iter()
            .filter(|f| f.buffer.is_some())
            .collect();
        let span = match (frames.first(), frames.last()) {
            (Some(a), Some(b)) => b.committed.since(a.committed),
            _ => Nanos::ZERO,
        };
        let secs = span.as_secs_f64();
        let presented = frames.iter().filter(|f| f.presented.is_some()).count();
        let rate = |n: usize| (secs > 0.0 && n > 1).then(|| (n - 1) as f64 / secs);
        ThroughputReport {
            commits: frames.len(),
            presented,
            discarded: frames
                .iter()
                .filter(|f| f.outcome() == FrameOutcome::Discarded)
                .count(),
            duration_ms: span.as_millis_f64(),
            commit_fps: rate(frames.len()),
            present_fps: rate(presented),
            commit_interval_ms: summary_ms(
                frames
                    .windows(2)
                    .map(|w| w[1].committed.since(w[0].committed)),
            ),
            gpu_ms: summary_ms(frames.iter().filter_map(|f| f.gpu_time)),
            ready_to_present_ms: summary_ms(frames.iter().filter_map(|f| {
                Some(
                    f.presented_time()?
                        .since(f.effective_ready())
                        .max(Nanos::ZERO),
                )
            })),
            explicit_sync_fraction: if frames.is_empty() {
                0.0
            } else {
                frames
                    .iter()
                    .filter(|f| f.sync == SyncKind::Explicit)
                    .count() as f64
                    / frames.len() as f64
            },
            vsync: frames
                .iter()
                .filter_map(|f| f.presented)
                .any(|p| p.flags & 1 != 0),
        }
    }
}
