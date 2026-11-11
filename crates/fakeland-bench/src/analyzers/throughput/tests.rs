//! Throughput on a synthetic unthrottled client.

use fakeland_trace::{Event, SyncKind};
use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{CommitOpts, OUT, SURF, Synth, at_ms};

#[test]
fn rates_and_gpu_time() {
    let mut s = Synth::new();
    for k in 0..=500u64 {
        let t = at_ms(k as f64 * 2.0);
        let c = s.commit(
            t,
            CommitOpts {
                sync: SyncKind::Explicit,
                ..CommitOpts::default()
            },
        );
        s.ready(c, t + Nanos::from_micros(1200));
        let time = t + Nanos::from_micros(1300);
        s.push(
            time,
            Event::Presented {
                surface: SURF,
                commit: c,
                output: OUT,
                seq: k,
                time,
                refresh: None,
                flags: 2,
            },
        );
    }
    let r = Throughput.analyze(&s.analysis());
    assert_eq!((r.commits, r.presented), (501, 501));
    assert!((r.commit_fps.unwrap() - 500.0).abs() < 1e-6);
    assert!((r.gpu_ms.unwrap().p50 - 1.2).abs() < 1e-9);
    assert!((r.ready_to_present_ms.unwrap().p50 - 0.1).abs() < 1e-9);
    assert_eq!(r.explicit_sync_fraction, 1.0);
    assert!(!r.vsync);
}
