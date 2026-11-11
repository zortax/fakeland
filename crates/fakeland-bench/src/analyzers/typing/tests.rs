//! Typing on synthetic traces.

use fakeland_trace::InputKind;
use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{Synth, at_ms};

/// Sends `n` keys `gap_ms` apart. The client handles one key per frame: it commits 4 ms after
/// a vblank with the oldest unhandled key and presents at the next vblank.
fn typed(n: u64, gap_ms: f64) -> crate::input::AnalysisInput {
    let mut s = Synth::new();
    let mut keys = Vec::new();
    for k in 0..n {
        let t = at_ms(20.0 + gap_ms * k as f64);
        s.input(t, InputKind::Key, None, "Shift_L pressed (keycode 50)");
        let id = s.input(t, InputKind::Key, None, "a pressed (keycode 38)");
        s.input(
            t + Nanos::from_millis(5),
            InputKind::Key,
            None,
            "a released (keycode 38)",
        );
        keys.push((t, id));
    }
    let mut next = 0;
    let mut seq = 0;
    while next < keys.len() {
        let commit_t = s.vblank(seq) + Nanos::from_millis(4);
        seq += 1;
        if keys[next].0 > commit_t {
            continue;
        }
        let c = s.frame(commit_t, seq);
        s.handled(c, commit_t, keys[next].1);
        next += 1;
    }
    s.analysis()
}

#[test]
fn slow_typing_has_no_backlog() {
    let r = Typing.analyze(&typed(10, 100.0));
    assert_eq!((r.keys, r.handled, r.presented), (10, 10, 10));
    assert_eq!(r.max_backlog, 0);
    let l = r.latency_ms.unwrap();
    assert!(l.min > 1.0 && l.max < 35.0, "{} {}", l.min, l.max);
    assert!(r.latency_slope_ms_per_key.unwrap().abs() < 2.0);
    assert!((r.keys_per_s.unwrap() - 10.0).abs() < 1e-6);
}

#[test]
fn bursts_build_a_backlog() {
    let r = Typing.analyze(&typed(20, 5.0));
    assert_eq!(r.keys, 20);
    assert!(r.max_backlog >= 12, "{}", r.max_backlog);
    assert!(r.latency_slope_ms_per_key.unwrap() > 10.0);
    assert!(
        r.per_key
            .windows(2)
            .all(|w| w[1].latency_ms >= w[0].latency_ms)
    );
}
