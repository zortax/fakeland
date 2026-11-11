//! The resize analyzer.

use fakeland_trace::frames::FrameOutcome;
use fakeland_trace::stats::median;
use fakeland_types::{Nanos, Timestamp};

use super::{ConfigureRecord, ResizeReport, configures_of};
use crate::analyzer::Analyzer;
use crate::analyzers::util::{logical_size, summary_ms};
use crate::input::AnalysisInput;

/// Measures how the target toplevel answers configures.
#[derive(Clone, Debug)]
pub struct Resize {
    /// The logical size difference that still counts as a match.
    pub tolerance_px: f64,
}

impl Default for Resize {
    fn default() -> Self {
        Self { tolerance_px: 0.5 }
    }
}

impl Analyzer for Resize {
    type Report = ResizeReport;

    fn name(&self) -> String {
        "resize".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> ResizeReport {
        let surface = input
            .target
            .as_ref()
            .map(|t| t.surface)
            .or_else(|| input.main_surface());
        let tl = &input.timeline;
        let frames = surface.map(|s| tl.frames(s)).unwrap_or_default();
        let records = surface
            .map(|s| configures_of(&input.trace, s, frames))
            .unwrap_or_default();
        let answered: Vec<_> = records
            .iter()
            .filter(|c| c.acked.is_some() && c.sized())
            .filter_map(|c| Some((c, c.buffer_size?)))
            .collect();
        let dw: Vec<f64> = answered
            .iter()
            .map(|(c, b)| b.0 - f64::from(c.width))
            .collect();
        let dh: Vec<f64> = answered
            .iter()
            .map(|(c, b)| b.1 - f64::from(c.height))
            .collect();
        let margin = (median(&dw).unwrap_or(0.0), median(&dh).unwrap_or(0.0));
        let fits = |w: i32, h: i32, b: (f64, f64)| {
            (b.0 - f64::from(w) - margin.0).abs() <= self.tolerance_px
                && (b.1 - f64::from(h) - margin.1).abs() <= self.tolerance_px
        };
        let matched = answered
            .iter()
            .filter(|(c, b)| fits(c.width, c.height, *b))
            .count();
        let mismatched_frames = frames
            .iter()
            .filter(|f| f.is_presented_frame())
            .filter_map(|f| {
                let serial = f.configure_serial?;
                let c = records.iter().rev().find(|c| c.serial <= serial)?;
                Some((c, logical_size(f.buffer.as_ref()?)))
            })
            .filter(|(c, b)| c.sized() && !fits(c.width, c.height, *b))
            .count();
        let live = || records.iter().filter(|c| !c.superseded);
        let storms = storms(&records);
        let storm_of = |t: Timestamp| storms.iter().position(|(a, b)| t >= *a && t <= *b);
        let in_storm = |t: Timestamp| storm_of(t).is_some();
        let missed_vblanks = surface
            .map(|s| {
                tl.presented_frames(s, None)
                    .windows(2)
                    .filter(|w| {
                        storm_of(w[0].time).is_some() && storm_of(w[0].time) == storm_of(w[1].time)
                    })
                    .filter_map(|w| w[1].vblanks_missed)
                    .sum()
            })
            .unwrap_or(0);
        ResizeReport {
            configures: records.len(),
            acked: records.iter().filter(|c| c.acked.is_some()).count(),
            superseded: records.iter().filter(|c| c.superseded).count(),
            matched,
            configure_to_ack_ms: summary_ms(live().filter_map(|c| Some(c.acked?.since(c.sent)))),
            configure_to_commit_ms: summary_ms(
                live().filter_map(|c| Some(c.committed?.since(c.sent))),
            ),
            configure_to_present_ms: summary_ms(
                live().filter_map(|c| Some(c.presented?.since(c.sent))),
            ),
            margin,
            mismatched_frames,
            missed_vblanks,
            discarded: frames
                .iter()
                .filter(|f| f.outcome() == FrameOutcome::Discarded)
                .filter(|f| f.discarded.is_some_and(in_storm))
                .count(),
            storm_ms: storms
                .iter()
                .map(|(a, b)| b.since(*a).as_millis_f64())
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))),
            records,
        }
    }
}

/// Configures closer than this belong to one storm.
const STORM_GAP: Nanos = Nanos::from_millis(100);

/// Returns the spans of configure storms: runs of configures less than [`STORM_GAP`] apart, from
/// the first send to the last presented answer. A single configure is a storm of one.
fn storms(records: &[ConfigureRecord]) -> Vec<(Timestamp, Timestamp)> {
    let mut out: Vec<(Timestamp, Timestamp, Timestamp)> = Vec::new();
    for c in records {
        let end = c.presented.unwrap_or(c.sent).max(c.sent);
        match out.last_mut() {
            Some((_, last_sent, stop)) if c.sent.since(*last_sent) < STORM_GAP => {
                *last_sent = c.sent;
                *stop = (*stop).max(end);
            }
            _ => out.push((c.sent, c.sent, end)),
        }
    }
    out.into_iter().map(|(a, _, b)| (a, b)).collect()
}
