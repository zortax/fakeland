//! Scroll smoothness on synthetic frames cut from a tall texture.

use fakeland_trace::InputKind;
use fakeland_types::Nanos;

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::{Synth, capture_of, captured, texture};

/// Scrolls 10 px per event at 120 Hz for `events` events, starting 100 ms in, on a 60 Hz client
/// that commits 5 ms after each vblank. Frames whose index is a multiple of `jank_every` present
/// two refreshes late.
fn scroll(events: u64, jank_every: u64) -> crate::input::AnalysisInput {
    scroll_over(texture(160, 4000, 3), events, jank_every, 1)
}

/// `scroll` over the content `tall`, which moves `gain` pixels per scroll pixel.
fn scroll_over(
    tall: fakeland_vision::Image,
    events: u64,
    jank_every: u64,
    gain: i32,
) -> crate::input::AnalysisInput {
    let mut s = Synth::new();
    let mut sent = Vec::new();
    for k in 0..events {
        let t = crate::testing::at_ms(100.0 + k as f64 * 1000.0 / 120.0);
        s.input(
            t,
            InputKind::PointerAxis,
            Some((80.0, 100.0)),
            "wheel dx=0 dy=10 v120=(0,80)",
        );
        sent.push(t);
    }
    let mut frames = Vec::new();
    let mut seq = 0u64;
    let mut index = 0u64;
    while seq < 120 {
        index += 1;
        let jank = jank_every > 0 && index.is_multiple_of(jank_every);
        let commit_t = s.vblank(seq) + Nanos::from_millis(5);
        let offset = sent.iter().filter(|t| **t <= commit_t).count() as i32 * 10 * gain;
        seq += if jank { 3 } else { 1 };
        let c = s.frame(commit_t, seq);
        let img = tall
            .crop(fakeland_types::Rect::new(0, offset, 160, 200))
            .expect("crop");
        frames.push(captured(img, c, s.vblank(seq), seq));
    }
    s.analysis().with_frames(vec![capture_of(frames)])
}

#[test]
fn even_scroll_is_smooth() {
    let r = ScrollSmoothness::default().analyze(&scroll(120, 0));
    assert_eq!(r.inputs, 120);
    assert_eq!(r.input_total_px, 1200.0);
    assert_eq!(
        r.rejected_steps,
        0,
        "{:?}",
        r.steps.iter().find(|s| s.measured_px.is_none())
    );
    assert_eq!(r.stalls, 0);
    assert_eq!(r.double_steps, 0);
    assert!(r.relative_error.unwrap() < 0.02, "{:?}", r.relative_error);
    assert!(
        r.smoothness_score.unwrap() > 95.0,
        "{:?}",
        r.smoothness_score
    );
    assert!((r.tracking_ratio.unwrap() - 1.0).abs() < 0.02);
    assert!(
        (r.measured_total_px - 1200.0).abs() < 5.0,
        "{}",
        r.measured_total_px
    );
    let v = r.velocity_px_s.unwrap();
    assert!((v.p50 - 1200.0).abs() < 30.0, "{}", v.p50);
    assert!(
        r.velocity_error_px_s.unwrap() < 30.0,
        "{:?}",
        r.velocity_error_px_s
    );
    let lat = r.start_latency_ms.unwrap();
    assert!(lat > 5.0 && lat < 35.0, "{lat}");
    assert!(r.samples().unwrap() > 50);
}

#[test]
fn late_frames_show_as_stalls_and_double_steps() {
    let smooth = ScrollSmoothness::default().analyze(&scroll(120, 0));
    let r = ScrollSmoothness::default().analyze(&scroll(120, 8));
    let janks = r
        .steps
        .iter()
        .filter(|s| s.active && s.vblanks_missed > 0)
        .count();
    assert!(janks >= 4, "{janks}");
    assert_eq!(r.missed_vblanks, 2 * janks as u64);
    assert!(r.stalls >= 2 * janks as u64);
    assert!(r.double_steps >= janks, "{} {janks}", r.double_steps);
    assert!(r.smoothness_score.unwrap() < smooth.smoothness_score.unwrap() - 10.0);
    assert!(
        r.relative_error.unwrap() < 0.05,
        "late frames still show the right content"
    );
    let names: Vec<String> = r.metrics().into_iter().map(|m| m.name).collect();
    assert!(names.contains(&"stalls".to_string()));
}

#[test]
fn no_capture_gives_an_empty_report() {
    let r = ScrollSmoothness::default().analyze(&Synth::new().analysis());
    assert_eq!((r.frames, r.active_steps), (0, 0));
    assert_eq!(r.smoothness_score, None);
}

/// A list of 32 px rows: a border line, zebra stripes and a small text-like mark that differs
/// from row to row, so the content repeats every row with small changes.
fn rows(w: u32, h: u32) -> fakeland_vision::Image {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let row = y / 32;
        let in_row = y % 32;
        for x in 0..w {
            let base: u8 = if row % 2 == 0 { 255 } else { 246 };
            let text = (10..20).contains(&in_row)
                && ((12 + (row % 7) * 6..15 + (row % 7) * 6).contains(&x)
                    || (100 + (row * 5 % 11) * 4..103 + (row * 5 % 11) * 4).contains(&x));
            let v = if in_row == 31 {
                229
            } else if text {
                40
            } else {
                base
            };
            data.extend_from_slice(&[v, v, v, 255]);
        }
    }
    fakeland_vision::Image::from_rgba(w, h, data).expect("rows")
}

#[test]
fn a_long_step_over_periodic_rows_keeps_the_expected_period() {
    let tall = rows(160, 2000);
    let frame = |offset: i32, seq: u64| {
        let img = tall
            .crop(fakeland_types::Rect::new(0, offset, 160, 400))
            .expect("crop");
        captured(
            img,
            fakeland_types::CommitId(seq),
            crate::testing::at_ms(seq as f64 * 16.7),
            seq,
        )
    };
    let mut est = fakeland_vision::MotionEstimator::new();
    // A frame that misses a refresh moves 27 px, between one and two row heights of 32 px.
    for (step, previous) in [(27, 13.0), (40, 13.0), (-27, -13.0)] {
        let (a, b) = (frame(600, 1), frame(600 + step, 2));
        let px = super::measure::displacement(
            &mut est,
            &a,
            &b,
            fakeland_vision::Axis::Vertical,
            &[f64::from(step), 0.0, previous],
            200,
        )
        .expect("displacement");
        assert!(
            (px - f64::from(step)).abs() < 1.0,
            "step {step}: measured {px}"
        );
    }
}

#[test]
fn an_app_gain_over_periodic_rows_is_measured() {
    // Two or three content pixels per scroll pixel: 40 or 60 px per refresh over 32 px rows, so a
    // search near the unscaled 20 px step finds a row alias.
    for gain in [2, 3] {
        let r = ScrollSmoothness::default().analyze(&scroll_over(rows(160, 8000), 100, 0, gain));
        let g = f64::from(gain);
        assert!(
            (r.content_gain - g).abs() < 0.05,
            "{gain}: {}",
            r.content_gain
        );
        let tracking = r.tracking_ratio.unwrap();
        assert!((tracking - g).abs() < 0.05, "{gain}: {tracking}");
        assert_eq!(r.rejected_steps, 0, "{gain}");
        assert_eq!(r.stalls, 0, "{gain}");
        assert!(
            r.relative_error.unwrap() < 0.02,
            "{gain}: {:?}",
            r.relative_error
        );
        assert!(
            r.smoothness_score.unwrap() > 95.0,
            "{gain}: {:?}",
            r.smoothness_score
        );
    }
}

#[test]
fn a_one_to_one_scroll_has_unit_gain() {
    let r = ScrollSmoothness::default().analyze(&scroll_over(rows(160, 4000), 120, 0, 1));
    assert_eq!(r.content_gain, 1.0);
    assert!(
        (r.tracking_ratio.unwrap() - 1.0).abs() < 0.02,
        "{:?}",
        r.tracking_ratio
    );
    assert!(
        r.smoothness_score.unwrap() > 95.0,
        "{:?}",
        r.smoothness_score
    );
}
