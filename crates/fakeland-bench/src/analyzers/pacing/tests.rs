//! Frame pacing on synthetic traces.

use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::{CommitOpts, Synth};

/// Builds a client that commits 7 ms after each vblank. Frames whose id is a multiple of
/// `jank_every` take 25 ms more and present two refreshes later.
fn janky(frames: u64, jank_every: u64) -> Synth {
    let mut s = Synth::new();
    let mut seq = 0;
    for id in 1..=frames {
        let jank = jank_every > 0 && id.is_multiple_of(jank_every);
        let commit_t = s.vblank(seq) + Nanos::from_millis(if jank { 32 } else { 7 });
        seq += if jank { 3 } else { 1 };
        let c = s.frame(commit_t, seq);
        s.frame_id(c, commit_t, id);
    }
    s.vblanks(1..=seq, Nanos::from_micros(50));
    s
}

#[test]
fn steady_client_presents_every_refresh() {
    let input = janky(61, 0).analysis();
    let r = FramePacing::default().analyze(&input);
    assert_eq!(r.presented, 61);
    assert_eq!(r.commits, 61);
    assert!((r.fps.unwrap() - 60.0).abs() < 0.01, "{:?}", r.fps);
    assert_eq!(r.jank.count, 0);
    assert_eq!(r.missed_vblanks, 0);
    assert_eq!(r.mode, PresentMode::Vsync);
    assert!((r.cadence.presents_per_refresh.unwrap() - 1.0).abs() < 1e-3);
    assert!(r.jitter_ms.unwrap() < 1e-3);
    assert_eq!(r.fps_series.len(), 1);
    assert_eq!(r.fps_series[0].frames, 60);
    assert_eq!(r.samples(), Some(60));
    let names: Vec<String> = r.metrics().into_iter().map(|m| m.name).collect();
    assert!(names.contains(&"fps".to_string()));
    assert!(names.contains(&"jank_count".to_string()));
}

#[test]
fn detects_exactly_the_jank_frames() {
    let input = janky(100, 10).analysis();
    let r = FramePacing::default().analyze(&input);
    let ids: Vec<u64> = r.jank.frames.iter().filter_map(|f| f.frame_id).collect();
    assert_eq!(ids, (1..=10).map(|k| k * 10).collect::<Vec<_>>());
    assert_eq!(r.missed_vblanks, 20);
    assert_eq!(r.repeated_frames, 10);
    assert!((r.jank.median_interval_ms - 16.667).abs() < 0.01);
    assert!(
        r.jank
            .frames
            .iter()
            .all(|f| (f.interval_ms - 50.0).abs() < 0.01)
    );
    assert!(r.lows.unwrap().low_1pct_fps < 21.0);
}

#[test]
fn mailbox_client_is_detected() {
    let mut s = Synth::new();
    for seq in 1..=30u64 {
        let base = s.vblank(seq - 1);
        let a = s.commit(base + Nanos::from_millis(2), CommitOpts::default());
        let b = s.commit(base + Nanos::from_millis(9), CommitOpts::default());
        s.discard(a, base + Nanos::from_millis(9));
        s.present(b, seq);
    }
    let r = FramePacing::default().analyze(&s.analysis());
    assert_eq!(r.discarded, 30);
    assert_eq!(r.mode, PresentMode::Mailbox);
    assert!((r.cadence.commits_per_refresh.unwrap() - 2.0).abs() < 0.1);
}

#[test]
fn commit_timing_targets() {
    let mut s = Synth::new();
    for seq in 1..=10u64 {
        let target = s.vblank(seq) - Nanos::from_millis(2);
        let c = s.commit(
            s.vblank(seq - 1) + Nanos::from_millis(1),
            CommitOpts {
                target: Some(target),
                ..CommitOpts::default()
            },
        );
        // Every fifth frame presents one refresh late.
        s.present(c, if seq.is_multiple_of(5) { seq + 1 } else { seq });
    }
    let r = FramePacing::default().analyze(&s.analysis());
    let ct = r.commit_timing.unwrap();
    assert_eq!((ct.frames, ct.late, ct.early), (10, 2, 0));
    assert!((ct.error_ms.unwrap().p50 - 2.0).abs() < 1e-6);
}

#[test]
fn pooling_merges_iterations() {
    let a = janky(31, 10).analysis();
    let b = janky(31, 0).analysis();
    let pooled = FramePacing::default().pool(&[&a, &b]).unwrap();
    assert_eq!(pooled.presented, 62);
    assert_eq!(pooled.interval_ms.unwrap().count, 60);
    assert_eq!(pooled.jank.count, 3);
    assert!(FramePacing::default().pool(&[&a]).is_none());
}

#[test]
fn empty_trace_gives_an_empty_report() {
    let r = FramePacing::default().analyze(&Synth::new().analysis());
    assert_eq!(r.presented, 0);
    assert_eq!(r.fps, None);
    assert_eq!(r.mode, PresentMode::Unknown);
    assert_eq!(r.samples(), Some(0));
}
