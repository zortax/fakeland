//! Drag follow on synthetic frames cut from a texture.

use fakeland_trace::InputKind;
use fakeland_types::{Nanos, Rect, Timestamp};

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{Synth, at_ms, capture_of, captured, texture};

/// Drags 300 px right over 600 ms at 125 Hz. The 60 Hz client draws the content at the last
/// pointer position it saw when it commits, 5 ms after each vblank.
fn drag() -> crate::input::AnalysisInput {
    let mut s = Synth::new();
    let big = texture(800, 240, 9);
    s.input(
        at_ms(95.0),
        InputKind::PointerButton,
        Some((100.0, 100.0)),
        "button 272 pressed",
    );
    let mut motions: Vec<(Timestamp, f64)> = Vec::new();
    for k in 0..=75u64 {
        let t = at_ms(100.0 + k as f64 * 8.0);
        let x = 100.0 + 4.0 * k as f64;
        s.input(t, InputKind::PointerMotion, Some((x, 100.0)), "");
        motions.push((t, x));
    }
    s.input(
        at_ms(700.0),
        InputKind::PointerButton,
        Some((400.0, 100.0)),
        "button 272 released",
    );
    let mut frames = Vec::new();
    for seq in 1..=60u64 {
        let commit_t = s.vblank(seq - 1) + Nanos::from_millis(5);
        let x = motions
            .iter()
            .take_while(|(t, _)| *t <= commit_t)
            .last()
            .map_or(100.0, |m| m.1);
        let shift = (x - 100.0) as i32;
        let c = s.frame(commit_t, seq);
        // The content moves right with the pointer, so the view origin moves left.
        let img = big
            .crop(Rect::new(400 - shift, 20, 300, 200))
            .expect("crop");
        frames.push(captured(img, c, s.vblank(seq), seq));
    }
    s.analysis().with_frames(vec![capture_of(frames)])
}

#[test]
fn content_trails_the_pointer_by_the_pipeline() {
    let r = DragFollow::default().analyze(&drag());
    assert_eq!(r.motions, 76);
    assert!((r.distance_px - 300.0).abs() < 1e-9);
    assert_eq!(r.rejected_pairs, 0);
    assert!(r.samples.len() > 30, "{}", r.samples.len());
    let ms = r.lag_ms.unwrap();
    // The content shows the pointer at the commit, 11.7 ms before presentation, minus up to
    // one 8 ms report gap.
    assert!(ms.p50 > 10.0 && ms.p50 < 21.0, "{}", ms.p50);
    let px = r.lag_px.unwrap();
    assert!(px.p50 > 0.0 && px.max < 13.0, "{} {}", px.p50, px.max);
    assert!(r.final_error_px.unwrap() < 1.0, "{:?}", r.final_error_px);
}

#[test]
fn path_queries() {
    let t = |ms| at_ms(ms);
    let p = PointerPath::new(vec![
        (t(0.0), 0.0, 0.0),
        (t(10.0), 10.0, 0.0),
        (t(20.0), 20.0, 0.0),
    ]);
    let (dir, d) = p.direction().unwrap();
    assert_eq!((dir, d), ((1.0, 0.0), 20.0));
    assert_eq!(p.along_at(t(15.0), dir), 10.0);
    assert_eq!(p.time_reaching(15.0, dir), Some(t(15.0)));
    assert_eq!(p.time_reaching(25.0, dir), None);
    assert!(PointerPath::default().direction().is_none());
}
