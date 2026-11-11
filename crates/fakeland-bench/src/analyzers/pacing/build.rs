//! The frame pacing report from pacing samples.

use fakeland_trace::stats::{FpsLows, median};
use fakeland_types::Nanos;

use super::mode::infer;
use super::series::fps_series;
use super::{
    CadenceReport, CommitTimingReport, FramePacingReport, JankFrame, JankReport, PacingSamples,
};
use crate::analyzers::util::{ms_values, summary_ms};

/// Builds the report. `jank_factor` is the factor of the jank rule.
pub fn build(s: &PacingSamples, jank_factor: f64, series: bool) -> FramePacingReport {
    let intervals = s.intervals();
    let ms = ms_values(intervals.iter().copied());
    let interval_ms = summary_ms(intervals.iter().copied());
    let med = median(&ms);
    let pacing_error_ms =
        med.map(|m| ms.iter().map(|v| (v - m).abs()).sum::<f64>() / ms.len() as f64);
    let span_s = s.span.as_secs_f64();
    let fps = (span_s > 0.0 && !ms.is_empty()).then(|| ms.len() as f64 / span_s);
    let refresh_hz = s.period.map(|p| 1.0 / p.as_secs_f64());
    let threshold = med.map(|m| m * jank_factor);
    let jank_frames: Vec<JankFrame> = s
        .frames
        .iter()
        .filter_map(|f| {
            let iv = f.interval?.as_millis_f64();
            (iv > threshold?).then_some(JankFrame {
                commit: f.commit,
                frame_id: f.frame_id,
                presented: f.time,
                interval_ms: iv,
                vblanks_missed: f.vblanks_missed,
            })
        })
        .collect();
    let commit_s = s.commit_span.as_secs_f64();
    let commit_rate_hz =
        (commit_s > 0.0 && s.commits.len() > 1).then(|| (s.commits.len() - 1) as f64 / commit_s);
    let cadence = CadenceReport {
        commit_rate_hz,
        present_rate_hz: fps,
        refresh_hz,
        commits_per_refresh: commit_rate_hz.zip(refresh_hz).map(|(c, r)| c / r),
        presents_per_refresh: fps.zip(refresh_hz).map(|(f, r)| f / r),
    };
    let commit_timing = commit_timing(s);
    let times: Vec<_> = s.frames.iter().map(|f| f.time).collect();
    FramePacingReport {
        surface: s.surface,
        refresh_hz,
        commits: s.commits.len(),
        presented: s.frames.len(),
        discarded: s.discarded,
        pending: s.pending,
        duration_ms: s.span.as_millis_f64(),
        fps,
        jitter_ms: interval_ms.map(|i| i.stddev),
        interval_ms,
        pacing_error_ms,
        lows: FpsLows::from_intervals_ms(&ms),
        missed_vblanks: s.frames.iter().filter_map(|f| f.vblanks_missed).sum(),
        repeated_frames: s
            .frames
            .iter()
            .filter(|f| f.vblanks_missed.is_some_and(|m| m > 0))
            .count(),
        jank: JankReport {
            factor: jank_factor,
            median_interval_ms: med.unwrap_or(0.0),
            threshold_ms: threshold.unwrap_or(0.0),
            count: jank_frames.len(),
            fraction: if ms.is_empty() {
                0.0
            } else {
                jank_frames.len() as f64 / ms.len() as f64
            },
            frames: jank_frames,
        },
        cadence,
        discard_fraction: if s.commits.is_empty() {
            0.0
        } else {
            s.discarded as f64 / s.commits.len() as f64
        },
        mode: infer(
            s.commits.len(),
            s.discarded,
            s.fifo,
            s.frames.len(),
            s.without_vsync,
        ),
        commit_timing,
        fps_series: if series {
            fps_series(&times)
        } else {
            Vec::new()
        },
    }
}

fn commit_timing(s: &PacingSamples) -> Option<CommitTimingReport> {
    let errors: Vec<Nanos> = s
        .frames
        .iter()
        .filter_map(|f| Some(f.time.since(f.target?)))
        .collect();
    if errors.is_empty() {
        return None;
    }
    let late_limit = s.period.unwrap_or(Nanos::from_millis(16));
    Some(CommitTimingReport {
        frames: errors.len(),
        late: errors.iter().filter(|e| **e >= late_limit).count(),
        early: errors.iter().filter(|e| e.is_negative()).count(),
        error_ms: summary_ms(errors),
    })
}
