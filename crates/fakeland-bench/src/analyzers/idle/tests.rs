//! Idle efficiency on synthetic commits and captures.

use fakeland_trace::Event;
use fakeland_types::{Nanos, Rect};
use fakeland_vision::Image;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{CommitOpts, SURF, Synth, at_ms, captured, commit_capture_of};

fn square(x: u32) -> Image {
    let mut img = Image::filled(64, 48, [20, 20, 20, 255]);
    img.fill_rect(Rect::new(x as i32, 10, 8, 8), [250, 200, 0, 255]);
    img
}

/// Commits `n` frames at 60 Hz with a square at `x(k)` and damage `damage(k)`.
fn frames(
    n: u64,
    x: impl Fn(u64) -> u32,
    damage: impl Fn(u64) -> Vec<Rect<fakeland_types::Physical>>,
) -> crate::input::AnalysisInput {
    let mut s = Synth::new();
    s.size = (64, 48);
    let mut caps = Vec::new();
    for k in 0..n {
        let t = s.vblank(k) + Nanos::from_millis(3);
        let c = s.commit(
            t,
            CommitOpts {
                damage: damage(k),
                ..CommitOpts::default()
            },
        );
        s.present(c, k + 1);
        caps.push(captured(square(x(k)), c, s.vblank(k + 1), k + 1));
    }
    let mut input = s.analysis().with_frames(vec![commit_capture_of(caps)]);
    input.iteration.finished = at_ms(1000.0);
    input
}

#[test]
fn unchanged_full_damage_frames_are_wasted() {
    let r =
        IdleEfficiency::default().analyze(&frames(60, |_| 5, |_| vec![Rect::new(0, 0, 64, 48)]));
    assert_eq!(r.buffer_commits, 60);
    assert_eq!(r.unchanged_commits, Some(59));
    assert_eq!(r.wasted_commits, 59);
    assert!(r.wasted_fraction > 0.95);
    let d = r.damage.unwrap();
    assert_eq!(d.frames, 59);
    assert!(
        d.over_damage_ratio.is_none(),
        "nothing changed, so no ratio"
    );
    assert_eq!(d.unchanged_damage_fraction.unwrap().p50, 1.0);
    assert!((r.commits_per_s - 60.0).abs() < 0.5);
}

#[test]
fn exact_and_short_damage() {
    let moving = |k: u64| 4 + 2 * k as u32;
    let exact = frames(10, moving, |k| {
        let x = 4 + 2 * k as i32;
        vec![Rect::new(x - 2, 10, 10, 8)]
    });
    let r = IdleEfficiency::default().analyze(&exact);
    let d = r.damage.unwrap();
    assert_eq!(r.wasted_commits, 0);
    assert_eq!(d.under_damage_pixels, 0);
    assert!((d.over_damage_ratio.unwrap().p50 - 80.0 / 32.0).abs() < 1e-9);
    let short = frames(10, moving, |_| vec![Rect::new(0, 0, 2, 2)]);
    let d = IdleEfficiency::default().analyze(&short).damage.unwrap();
    assert_eq!(d.under_damaged_frames, 9);
    assert!(d.under_damage_pixels >= 9 * 32);
}

#[test]
fn idle_client_and_suspended_spans() {
    let mut s = Synth::new();
    let c = s.frame(at_ms(5.0), 1);
    let _ = c;
    s.configure(at_ms(500.0), 2, 1280, 720, &["activated", "suspended"]);
    for k in 0..6u64 {
        let t = at_ms(520.0 + 16.0 * k as f64);
        s.commit(t, CommitOpts::default());
        s.push(
            t + Nanos::from_millis(1),
            Event::FrameCallbackSent {
                surface: SURF,
                time: t,
            },
        );
    }
    s.configure(at_ms(700.0), 3, 1280, 720, &["activated"]);
    let mut input = s.analysis();
    input.iteration.finished = at_ms(1000.0);
    let r = IdleEfficiency::default().analyze(&input);
    assert_eq!(r.unchanged_commits, None);
    assert!((r.suspended.duration_ms - 200.0).abs() < 1e-6);
    assert_eq!(r.suspended.commits, 6);
    assert_eq!(r.suspended.callbacks_requested, 6);
    assert_eq!(r.suspended.callbacks_sent, 6);
    assert_eq!(r.buffer_commits, 7);
}
