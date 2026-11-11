//! The frame cost analyzer.

use fakeland_trace::{FrameRecord, Summary, SyncKind};
use fakeland_types::{Nanos, Timestamp};

use super::FrameCostReport;
use crate::analyzer::Analyzer;
use crate::analyzers::util::{selected_roots, summary_ms, tree_meters};
use crate::input::AnalysisInput;

/// The default damage fraction above which a frame counts as a full repaint.
pub const FULL_DAMAGE: f64 = 0.9;

/// Measures what each frame of the target's main surface costs: CPU time of the target tree per
/// presented frame, GPU fence time from commit to ready, and the damaged share of the buffer.
#[derive(Default, Clone, Debug)]
pub struct FrameCost {
    /// The damage fraction above which a frame counts as a full repaint; `None` means 0.9.
    pub full_damage: Option<f64>,
}

impl Analyzer for FrameCost {
    type Report = FrameCostReport;

    fn name(&self) -> String {
        "frame_cost".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> FrameCostReport {
        let (start, end) = input.span();
        let frames: Vec<&FrameRecord> = input
            .main_surface()
            .map(|s| {
                input
                    .timeline
                    .frames_in(s, start..=end)
                    .filter(|f| f.has_content())
                    .collect()
            })
            .unwrap_or_default();
        let presented: Vec<Timestamp> = frames
            .iter()
            .filter(|f| f.is_presented_frame())
            .filter_map(|f| f.presented_time())
            .collect();
        let (cpu_ms, cpu_frames) = cpu_window(input, &presented);
        // A buffer without a fence counts as ready at commit, so its commit-to-ready time says
        // nothing about the GPU. Only fenced frames give GPU times.
        let gpu: Vec<Nanos> = frames
            .iter()
            .filter(|f| f.sync != SyncKind::None)
            .filter_map(|f| f.gpu_time)
            .collect();
        let span = end.since(start).as_secs_f64();
        let fence_total: f64 = gpu.iter().map(|n| n.as_millis_f64()).sum();
        let damage: Vec<f64> = frames.iter().filter_map(|f| damage_fraction(f)).collect();
        FrameCostReport {
            frames: presented.len(),
            commits: frames.len(),
            span_ms: span * 1e3,
            cpu_ms,
            cpu_frames,
            cpu_ms_per_frame: cpu_ms
                .filter(|_| cpu_frames > 0)
                .map(|ms| ms / cpu_frames as f64),
            gpu_fence_ms: summary_ms(gpu.iter().copied()),
            gpu_fence_ms_per_s: (span > 0.0 && !gpu.is_empty()).then(|| fence_total / span),
            damage_fraction: Summary::of(&damage),
            full_damage_frames: damage
                .iter()
                .filter(|d| **d > self.full_damage.unwrap_or(FULL_DAMAGE))
                .count(),
        }
    }
}

/// Returns the CPU time of the trees under test from their first to their last detail sample, and
/// the presented frames inside that window.
fn cpu_window(input: &AnalysisInput, presented: &[Timestamp]) -> (Option<f64>, usize) {
    let meters = tree_meters(input);
    let roots = selected_roots(
        input,
        meters
            .iter()
            .filter(|(_, m)| m.cpu_ns.is_some())
            .map(|(pid, _)| *pid),
    );
    let counters: Vec<_> = roots
        .iter()
        .filter_map(|pid| meters.get(pid)?.cpu_ns)
        .filter(|c| c.secs() > 0.0)
        .collect();
    let (Some(from), Some(to)) = (
        counters.iter().map(|c| c.first.0).min(),
        counters.iter().map(|c| c.last.0).max(),
    ) else {
        return (None, 0);
    };
    let ns: u64 = counters.iter().map(|c| c.delta()).sum();
    let frames = presented.iter().filter(|t| **t > from && **t <= to).count();
    (Some(ns as f64 / 1e6), frames)
}

/// Returns the damaged area over the buffer area, clipped to 1, of a frame with a buffer.
fn damage_fraction(f: &FrameRecord) -> Option<f64> {
    let b = f.buffer.as_ref()?;
    let area = i64::from(b.width) * i64::from(b.height);
    (area > 0).then(|| (f.damage_area as f64 / area as f64).clamp(0.0, 1.0))
}
