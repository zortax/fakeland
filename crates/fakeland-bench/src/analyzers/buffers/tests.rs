//! Buffers on synthetic commits.

use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::{CommitOpts, Synth};

#[test]
fn pool_reuse_in_flight_and_scale() {
    let mut s = Synth::with_output(60.0, 1.5);
    s.size = (100, 80);
    let mut prev = None;
    for k in 0..120u64 {
        let t = s.vblank(k) + Nanos::from_millis(2);
        let c = s.commit(
            t,
            CommitOpts {
                buffer_id: Some(k % 3),
                // Frames from 60 on render at scale 1 and get stretched.
                size: (k >= 60).then_some((100, 80)),
                viewport: Some((100, 80)),
                ..CommitOpts::default()
            },
        );
        s.present(c, k + 1);
        if let Some(p) = prev {
            s.release(p, s.vblank(k + 1));
        }
        prev = Some(c);
    }
    let r = Buffers.analyze(&s.analysis());
    assert_eq!(r.buffer_commits, 120);
    assert_eq!(r.distinct_buffers, 3);
    assert_eq!(r.late_allocations, 0);
    assert_eq!(r.churn_per_s, Some(0.0));
    assert_eq!(r.max_in_flight, 2);
    assert_eq!(r.scale.native, 60);
    assert_eq!(r.scale.upscaled, 60);
    assert_eq!(r.kinds["shm"], 120);
    assert_eq!(r.formats[0].fourcc, "XR24");
    assert_eq!(r.distinct_sizes, 2);
    assert!(r.metrics().iter().any(|m| m.name == "churn_per_s"));
}

#[test]
fn fresh_buffers_count_as_churn() {
    let mut s = Synth::new();
    for k in 0..180u64 {
        let c = s.commit(
            s.vblank(k) + Nanos::from_millis(2),
            CommitOpts {
                buffer_id: Some(k),
                ..CommitOpts::default()
            },
        );
        s.present(c, k + 1);
    }
    let r = Buffers.analyze(&s.analysis());
    assert_eq!(r.distinct_buffers, 180);
    let churn = r.churn_per_s.unwrap();
    assert!((churn - 60.0).abs() < 2.0, "{churn}");
}
