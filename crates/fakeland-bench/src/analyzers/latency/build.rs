//! The latency report from probe results.

use fakeland::ProbeResult;
use fakeland_trace::stats::{ConfidenceInterval, Histogram, median};
use fakeland_types::Nanos;

use super::phase::by_phase;
use super::{LatencyReport, LatencySample};
use crate::analyzers::util::summary_ms;

/// The resamples of the median intervals.
const RESAMPLES: usize = 2000;

/// Builds the report of some probe results.
pub fn build(
    label: Option<String>,
    results: &[&ProbeResult],
    period: Option<Nanos>,
    phase_bins: u32,
    seed: u64,
) -> LatencyReport {
    let stage =
        |f: fn(&ProbeResult) -> Option<Nanos>| summary_ms(results.iter().filter_map(|r| f(r)));
    let totals: Vec<f64> = results
        .iter()
        .filter_map(|r| r.stages.total)
        .map(Nanos::as_millis_f64)
        .collect();
    let i2c: Vec<f64> = results
        .iter()
        .filter_map(|r| r.stages.input_to_commit)
        .map(Nanos::as_millis_f64)
        .collect();
    let ci = |v: &[f64], salt: u64| {
        ConfidenceInterval::bootstrap(
            v,
            |s| median(s).unwrap_or(f64::NAN),
            RESAMPLES,
            0.95,
            seed ^ salt,
        )
    };
    let pairs: Vec<(Nanos, f64)> = results
        .iter()
        .filter_map(|r| Some((r.phase_in_frame?, r.stages.total?.as_millis_f64())))
        .collect();
    let histogram = histogram(&totals);
    LatencyReport {
        label,
        repeats: results.len(),
        detected: results
            .iter()
            .filter(|r| r.detected_commit.is_some())
            .count(),
        presented: results.iter().filter(|r| r.presented.is_some()).count(),
        timed_out: results.iter().filter(|r| r.timed_out).count(),
        total_ms: summary_ms(results.iter().filter_map(|r| r.stages.total)),
        input_to_commit_ms: stage(|r| r.stages.input_to_commit),
        commit_to_ready_ms: stage(|r| r.stages.commit_to_ready),
        ready_to_latch_ms: stage(|r| r.stages.ready_to_latch),
        latch_to_present_ms: stage(|r| r.stages.latch_to_present),
        total_median_ci: ci(&totals, 1),
        input_to_commit_median_ci: ci(&i2c, 2),
        by_phase: period
            .map(|p| by_phase(&pairs, p, phase_bins))
            .unwrap_or_default(),
        histogram,
        readback_cost_ms: summary_ms(
            results
                .iter()
                .filter(|r| r.readbacks > 0)
                .map(|r| r.readback_cost),
        ),
        samples: results
            .iter()
            .map(|r| LatencySample {
                repeat: r.iteration,
                phase_ms: r.phase_in_frame.map(Nanos::as_millis_f64),
                input_to_commit_ms: r.stages.input_to_commit.map(Nanos::as_millis_f64),
                total_ms: r.stages.total.map(Nanos::as_millis_f64),
                timed_out: r.timed_out,
            })
            .collect(),
    }
}

fn histogram(totals: &[f64]) -> Option<Histogram> {
    let lo = totals.iter().copied().fold(f64::INFINITY, f64::min).floor();
    let hi = totals
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .floor()
        + 1.0;
    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }
    let bins = ((hi - lo) as usize).clamp(1, 1000);
    Histogram::linear(lo, hi, bins).ok().map(|h| h.fill(totals))
}
