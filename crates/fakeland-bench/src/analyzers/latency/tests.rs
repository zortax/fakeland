//! Latency on synthetic probe results.

use fakeland::{ProbeReport, ProbeResult, ProbeStages};
use fakeland_types::{CommitId, Nanos, Timestamp};

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::Synth;

/// A repeat at phase `phase_ms` with a 7 ms client and a 60 Hz display that latches 1 ms before
/// each vblank.
fn repeat(i: u32, phase_ms: f64) -> ProbeResult {
    let period = 1000.0 / 60.0;
    let t0 = Timestamp(1_000_000_000) + Nanos::from_millis_f64(phase_ms);
    let committed = t0 + Nanos::from_millis(7);
    let since = phase_ms + 7.0;
    let mut vb = (since / period).ceil() * period;
    if vb - since < 1.0 {
        vb += period;
    }
    let presented = Timestamp(1_000_000_000) + Nanos::from_millis_f64(vb);
    let latched = presented - Nanos::from_millis(1);
    ProbeResult {
        iteration: i,
        input_sent: Some(t0),
        phase_in_frame: Some(Nanos::from_millis_f64(phase_ms)),
        detected_commit: Some(CommitId(u64::from(i) + 1)),
        committed: Some(committed),
        ready: Some(committed),
        latched: Some(latched),
        presented: Some(presented),
        stages: ProbeStages::compute(
            Some(t0),
            Some(committed),
            Some(committed),
            Some(latched),
            Some(presented),
        ),
        readbacks: 2,
        readback_cost: Nanos::from_micros(300),
        ..ProbeResult::default()
    }
}

fn report(n: u32, label: Option<&str>) -> ProbeReport {
    let period = 1000.0 / 60.0;
    ProbeReport {
        label: label.map(String::from),
        detector: "pixels_changed".into(),
        results: (0..n)
            .map(|i| repeat(i, period * (f64::from(i % 8) + 0.5) / 8.0))
            .collect(),
    }
}

#[test]
fn stages_and_phase_distribution() {
    let mut timed_out = report(16, None);
    timed_out.results.push(ProbeResult {
        iteration: 16,
        timed_out: true,
        ..ProbeResult::default()
    });
    let input = Synth::new().analysis().with_probes(vec![timed_out]);
    let r = Latency::default().analyze(&input);
    assert_eq!(
        (r.repeats, r.detected, r.presented, r.timed_out),
        (17, 16, 16, 1)
    );
    let i2c = r.input_to_commit_ms.unwrap();
    assert!((i2c.p50 - 7.0).abs() < 1e-6 && (i2c.max - 7.0).abs() < 1e-6);
    assert!((r.latch_to_present_ms.unwrap().p50 - 1.0).abs() < 1e-6);
    let total = r.total_ms.unwrap();
    assert!(total.min > 8.0 && total.max < 8.0 + 1000.0 / 60.0 + 1e-6);
    assert_eq!(r.by_phase.len(), 8);
    assert!(r.by_phase.iter().all(|b| b.count == 2));
    // A later phase leaves less time to the latch until the frame misses it.
    let p = |i: usize| r.by_phase[i].total_p50_ms.unwrap();
    assert!((p(0) - (1000.0 / 60.0) * 15.0 / 16.0).abs() < 1e-3);
    assert!(p(3) < p(0) && p(4) > p(3), "{} {} {}", p(0), p(3), p(4));
    let ci = r.total_median_ci.unwrap();
    assert!(ci.lo <= ci.estimate && ci.estimate <= ci.hi);
    let h = r.histogram.as_ref().unwrap();
    assert_eq!(h.total, 16);
    assert_eq!(r.samples.len(), 17);
    assert!((r.readback_cost_ms.unwrap().p50 - 0.3).abs() < 1e-9);
    let m = r.metrics();
    assert_eq!(m.iter().find(|m| m.name == "timeouts").unwrap().value, 1.0);
    assert_eq!(r.samples(), Some(16));
}

#[test]
fn labels_and_skips_select_results() {
    let input = Synth::new()
        .analysis()
        .with_probes(vec![report(4, Some("hover")), report(6, Some("click"))]);
    let hover = Latency::default().label("hover").analyze(&input);
    assert_eq!(hover.repeats, 4);
    assert_eq!(Latency::default().label("hover").name(), "latency.hover");
    let skipped = Latency::default().skip_repeats(2).analyze(&input);
    assert_eq!(skipped.repeats, 6);
}

#[test]
fn pooling_combines_iterations() {
    let a = Synth::new().analysis().with_probes(vec![report(5, None)]);
    let b = Synth::new().analysis().with_probes(vec![report(7, None)]);
    let pooled = Latency::default().pool(&[&a, &b]).unwrap();
    assert_eq!(pooled.repeats, 12);
    assert_eq!(pooled.total_ms.unwrap().count, 12);
}

#[test]
fn no_probes_give_an_empty_report() {
    let r = Latency::default().analyze(&Synth::new().analysis());
    assert_eq!(r.repeats, 0);
    assert!(r.total_ms.is_none() && r.histogram.is_none());
    assert_eq!(r.samples(), Some(0));
}
