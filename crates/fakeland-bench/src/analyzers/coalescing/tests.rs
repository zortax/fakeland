//! Input coalescing on a synthetic pointer glide.

use fakeland_trace::InputKind;
use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{Synth, at_ms};

#[test]
fn a_1khz_mouse_on_a_60hz_client() {
    let mut s = Synth::new();
    let mut ids = Vec::new();
    for k in 0..500u64 {
        let t = at_ms(1.0 + k as f64);
        ids.push((
            t,
            s.input(t, InputKind::PointerMotion, Some((k as f64, 0.0)), ""),
        ));
        s.input(t, InputKind::PointerFrame, None, "");
    }
    for seq in 0..31u64 {
        let commit_t = s.vblank(seq) + Nanos::from_millis(4);
        let c = s.frame(commit_t, seq + 1);
        for (t, id) in &ids {
            if *t <= commit_t && *t + s.period > commit_t {
                s.handled(c, commit_t, *id);
            }
        }
    }
    let r = InputCoalescing.analyze(&s.analysis());
    assert_eq!(r.inputs, 500, "frames do not count");
    assert_eq!(r.by_kind["pointer_motion"], 500);
    let pf = r.per_frame.unwrap();
    assert!((pf.p50 - 16.5).abs() < 1.0, "{}", pf.p50);
    assert!(r.max_per_frame <= 17);
    assert!((r.input_rate_hz.unwrap() - 1000.0).abs() < 1.0);
}
