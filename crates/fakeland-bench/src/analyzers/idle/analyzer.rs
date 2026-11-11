//! The idle efficiency analyzer.

use std::collections::HashMap;

use fakeland::{CaptureSpec, CapturedFrame};
use fakeland_trace::{Event, FrameRecord, Summary};
use fakeland_types::{CommitId, Timestamp};
use fakeland_vision::{damage_accuracy, diff};

use super::damage::damage_in_capture;
use super::suspended::suspended_spans;
use super::{DamageStats, IdleReport, SuspendStats};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures wasted work of the target's main surface. A capture of every commit of the whole
/// window adds pixel checks and damage accuracy.
#[derive(Clone, Debug, Default)]
pub struct IdleEfficiency {
    /// A channel delta above this value counts as a change.
    pub threshold: u8,
}

impl IdleEfficiency {
    fn capture<'a>(&self, input: &'a AnalysisInput) -> Option<&'a [CapturedFrame]> {
        input
            .frames
            .iter()
            .find(|c| matches!(c.spec, CaptureSpec::EveryCommit { .. }))
            .map(|c| c.frames.as_slice())
    }
}

impl Analyzer for IdleEfficiency {
    type Report = IdleReport;

    fn name(&self) -> String {
        "idle_efficiency".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> IdleReport {
        let tl = &input.timeline;
        let (start, end) = input.span();
        let surface = input.main_surface();
        let frames: Vec<&FrameRecord> = surface
            .map(|s| tl.frames(s).iter().collect())
            .unwrap_or_default();
        let buffers: Vec<&FrameRecord> = frames
            .iter()
            .copied()
            .filter(|f| f.buffer.is_some())
            .collect();
        let secs = end.since(start).as_secs_f64().max(1e-9);
        let empty = buffers.iter().filter(|f| f.damage_area == 0).count();
        let capture = self.capture(input);
        let by_commit: HashMap<CommitId, &FrameRecord> =
            frames.iter().map(|f| (f.commit, *f)).collect();
        let mut unchanged = None;
        let mut damage = None;
        if let Some(cap) = capture {
            let mut same = 0;
            let mut over = Vec::new();
            let mut unchanged_damage = Vec::new();
            let mut under_px = 0;
            let mut under_frames = 0;
            let mut compared = 0;
            for pair in cap.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                let Ok(d) = diff(&a.image, &b.image, None, self.threshold) else {
                    continue;
                };
                if d.changed_pixels == 0 {
                    same += 1;
                }
                let Some(rec) = by_commit.get(&b.commit) else {
                    continue;
                };
                let Some(buffer) = &rec.buffer else { continue };
                let rects = damage_in_capture(&rec.damage, buffer, b);
                let Ok(acc) = damage_accuracy(&a.image, &b.image, &rects, self.threshold) else {
                    continue;
                };
                compared += 1;
                if acc.changed_area > 0 {
                    over.push(acc.over_damage_ratio);
                }
                if acc.reported_area > 0 {
                    unchanged_damage.push(acc.unchanged_damage_fraction);
                }
                under_px += acc.under_damage_pixels;
                if acc.under_damage_pixels > 0 {
                    under_frames += 1;
                }
            }
            unchanged = Some(same);
            damage = Some(DamageStats {
                frames: compared,
                over_damage_ratio: Summary::of(&over),
                unchanged_damage_fraction: Summary::of(&unchanged_damage),
                under_damage_pixels: under_px,
                under_damaged_frames: under_frames,
            });
        }
        let wasted = unchanged.unwrap_or(empty);
        let callbacks = frames.iter().filter(|f| f.frame_callback).count();
        let spans = surface
            .map(|s| suspended_spans(&input.trace, s, end))
            .unwrap_or_default();
        let inside = |t: Timestamp| spans.iter().any(|(a, b)| t >= *a && t < *b);
        let sent = input
            .trace
            .records
            .iter()
            .filter(|r| matches!(&r.event, Event::FrameCallbackSent { surface: s, .. } if Some(*s) == surface))
            .filter(|r| inside(r.t))
            .count();
        IdleReport {
            duration_ms: secs * 1000.0,
            commits: frames.len(),
            buffer_commits: buffers.len(),
            commits_per_s: buffers.len() as f64 / secs,
            empty_damage_commits: empty,
            unchanged_commits: unchanged,
            wasted_commits: wasted,
            wasted_fraction: if buffers.is_empty() {
                0.0
            } else {
                wasted as f64 / buffers.len() as f64
            },
            callbacks_requested: callbacks,
            callbacks_per_s: callbacks as f64 / secs,
            damage,
            suspended: SuspendStats {
                duration_ms: spans.iter().map(|(a, b)| b.since(*a).as_millis_f64()).sum(),
                commits: buffers.iter().filter(|f| inside(f.committed)).count(),
                callbacks_requested: frames
                    .iter()
                    .filter(|f| f.frame_callback && inside(f.committed))
                    .count(),
                callbacks_sent: sent,
            },
        }
    }
}
