//! Frame cost on synthetic traces.

use fakeland_trace::{Event, SyncKind};
use fakeland_types::{Nanos, ProcessId, Rect};

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::{CommitOpts, Synth, at_ms};

fn detail(cpu_ns: u64) -> Event {
    Event::ProcessDetail {
        pid: ProcessId(100),
        cpu_ns_total: cpu_ns,
        ctx_switches_total: 0,
        pss_bytes: None,
        private_bytes: None,
        swap_bytes: None,
        hwm_bytes: None,
    }
}

fn metric(r: &FrameCostReport, name: &str) -> f64 {
    r.metrics()
        .into_iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no metric {name}"))
        .value
}

/// Twenty frames at 60 Hz: every fourth frame repaints the whole 1280x720 buffer, the others a
/// 128x72 square. Frame `k` waits `k % 5 + 1` ms on its fence.
fn trace() -> Synth {
    let mut s = Synth::new();
    for k in 0..20u64 {
        let t = s.vblank(k) + Nanos::from_millis(4);
        let damage = if k % 4 == 0 {
            Rect::new(0, 0, 1280, 720)
        } else {
            Rect::new(0, 0, 128, 72)
        };
        let c = s.commit(
            t,
            CommitOpts {
                damage: vec![damage],
                sync: SyncKind::Explicit,
                ..CommitOpts::default()
            },
        );
        s.ready(c, t + Nanos::from_millis(k as i64 % 5 + 1));
        s.present(c, k + 1);
    }
    s.vblanks(0..=21, Nanos::ZERO);
    // The tree runs 2 ms per frame period: samples at 0 ms and at 200 ms, 12 vblanks apart.
    s.push(at_ms(0.0), detail(1_000_000_000));
    s.push(s.vblank(12), detail(1_000_000_000 + 24_000_000));
    s
}

#[test]
fn costs_per_frame() {
    let r = FrameCost::default().analyze(&trace().analysis());
    assert_eq!((r.frames, r.commits), (20, 20));
    // Vblanks 1 to 12 present inside the CPU sample window.
    assert_eq!(r.cpu_frames, 12);
    assert!((r.cpu_ms.unwrap() - 24.0).abs() < 1e-9);
    assert!((metric(&r, "cpu_ms_per_frame") - 2.0).abs() < 1e-9);
    assert!((metric(&r, "gpu_fence_ms_p50") - 3.0).abs() < 1e-9);
    assert!(metric(&r, "gpu_fence_ms_p95") >= 4.0);
    // 60 ms of fence time over the iteration span.
    let span_s = r.span_ms / 1e3;
    assert!((metric(&r, "gpu_fence_ms_per_s") - 60.0 / span_s).abs() < 1e-9);
    assert!((metric(&r, "damage_fraction_p50") - 0.01).abs() < 1e-9);
    let mean = (5.0 + 15.0 * 0.01) / 20.0;
    assert!((metric(&r, "damage_fraction_mean") - mean).abs() < 1e-9);
    assert_eq!(metric(&r, "full_damage_frames"), 5.0);
    assert_eq!(metric(&r, "frames"), 20.0);
    assert_eq!(r.metrics().len(), 8);
    assert_eq!(r.samples(), Some(20));
    let back: FrameCostReport = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
    let strict = FrameCost {
        full_damage: Some(0.005),
    };
    assert_eq!(strict.analyze(&trace().analysis()).full_damage_frames, 20);
}

#[test]
fn damage_clips_and_frames_without_samples_have_no_cpu_cost() {
    let mut s = Synth::new();
    let c = s.commit(
        at_ms(5.0),
        CommitOpts {
            damage: vec![Rect::new(0, 0, 1280, 720), Rect::new(0, 0, 4000, 4000)],
            ..CommitOpts::default()
        },
    );
    s.present(c, 1);
    let r = FrameCost::default().analyze(&s.analysis());
    assert_eq!(r.frames, 1);
    assert_eq!(r.cpu_ms_per_frame, None);
    assert_eq!(r.gpu_fence_ms, None);
    assert_eq!(r.damage_fraction.unwrap().max, 1.0);
    assert_eq!(r.full_damage_frames, 1);
    let empty = FrameCost::default().analyze(&Synth::new().analysis());
    assert_eq!((empty.frames, empty.commits), (0, 0));
    assert_eq!(empty.samples(), None);
    assert_eq!(empty.metrics().len(), 1);
}

#[test]
fn frames_without_a_fence_give_no_gpu_time() {
    let mut s = Synth::new();
    for k in 0..4u64 {
        let t = s.vblank(k) + Nanos::from_millis(4);
        let c = s.commit(t, CommitOpts::default());
        s.ready(c, t);
        s.present(c, k + 1);
    }
    s.vblanks(0..=5, Nanos::ZERO);
    let r = FrameCost::default().analyze(&s.analysis());
    assert_eq!(r.frames, 4);
    assert_eq!(r.gpu_fence_ms, None);
    assert_eq!(r.gpu_fence_ms_per_s, None);
}
