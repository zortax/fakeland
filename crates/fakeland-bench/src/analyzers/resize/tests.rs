//! Resize on a synthetic configure storm.

use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{CommitOpts, Synth};

/// Sends ten configures, one per refresh, growing by 10 px. The client acks each 2 ms later and
/// commits the new size 6 ms after the configure, except for configure 5, which it skips, and
/// configure 8, whose first frame still has the old size.
fn storm() -> crate::input::AnalysisInput {
    let mut s = Synth::new();
    for k in 0..10u32 {
        let seq = u64::from(k);
        let t = s.vblank(seq) + Nanos::from_millis(1);
        let (w, h) = (400 + 10 * k as i32, 300 + 10 * k as i32);
        s.configure(t, k + 1, w, h, &["resizing"]);
        if k == 4 {
            continue;
        }
        s.ack(t + Nanos::from_millis(2), k + 1);
        let size = if k == 7 { (w - 10, h - 10) } else { (w, h) };
        let c = s.commit(
            t + Nanos::from_millis(6),
            CommitOpts {
                size: Some(size),
                configure_serial: Some(k + 1),
                ..CommitOpts::default()
            },
        );
        s.present(c, seq + 1);
    }
    s.analysis()
}

#[test]
fn configure_answers_and_mismatches() {
    let r = Resize::default().analyze(&storm());
    assert_eq!((r.configures, r.acked, r.superseded), (10, 9, 1));
    assert_eq!(r.matched, 8);
    assert_eq!(r.mismatched_frames, 1);
    assert_eq!(r.margin, (0.0, 0.0));
    let ack = r.configure_to_ack_ms.unwrap();
    assert!((ack.p50 - 2.0).abs() < 1e-6 && ack.count == 9);
    let commit = r.configure_to_commit_ms.unwrap();
    assert!((commit.p50 - 6.0).abs() < 1e-6);
    let present = r.configure_to_present_ms.unwrap();
    assert!(present.p50 > 6.0 && present.max < 17.0);
    assert_eq!(
        r.missed_vblanks, 1,
        "the skipped configure leaves one refresh without a frame"
    );
    assert!(r.storm_ms.unwrap() > 140.0);
}

#[test]
fn a_constant_margin_counts_as_a_match() {
    let mut s = Synth::new();
    for k in 0..4u32 {
        let t = s.vblank(u64::from(k)) + Nanos::from_millis(1);
        s.configure(t, k + 1, 500, 400, &[]);
        s.ack(t, k + 1);
        let c = s.commit(
            t + Nanos::from_millis(3),
            CommitOpts {
                size: Some((548, 448)),
                configure_serial: Some(k + 1),
                ..CommitOpts::default()
            },
        );
        s.present(c, u64::from(k) + 1);
    }
    let r = Resize::default().analyze(&s.analysis());
    assert_eq!(r.margin, (48.0, 48.0));
    assert_eq!((r.matched, r.mismatched_frames), (4, 0));
}

#[test]
fn idle_gaps_between_storms_do_not_count() {
    let mut s = Synth::new();
    for (k, seq) in [(0u32, 0u64), (1, 1), (2, 2), (3, 40)] {
        let t = s.vblank(seq) + Nanos::from_millis(1);
        s.configure(t, k + 1, 500 + k as i32, 400, &[]);
        s.ack(t, k + 1);
        let c = s.commit(
            t + Nanos::from_millis(3),
            CommitOpts {
                size: Some((500 + k as i32, 400)),
                configure_serial: Some(k + 1),
                ..CommitOpts::default()
            },
        );
        s.present(c, seq + 1);
    }
    let r = Resize::default().analyze(&s.analysis());
    assert_eq!(r.missed_vblanks, 0);
    assert!((r.storm_ms.unwrap() - (3.0 * 1000.0 / 60.0 - 1.0)).abs() < 1e-3);
}
