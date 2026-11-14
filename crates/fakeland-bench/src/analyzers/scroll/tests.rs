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

/// `scroll` over `tall` on a client that shows every refresh, where `offset` maps the frame index
/// and the scroll its commit consumed, in pixels, to the content offset of the frame.
fn played(
    tall: &fakeland_vision::Image,
    events: u64,
    offset: impl Fn(u64, i32) -> i32,
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
    for seq in 1..=120u64 {
        let commit_t = s.vblank(seq - 1) + Nanos::from_millis(5);
        let consumed = sent.iter().filter(|t| **t <= commit_t).count() as i32 * 10;
        let c = s.frame(commit_t, seq);
        let img = tall
            .crop(fakeland_types::Rect::new(
                0,
                offset(seq, consumed),
                160,
                200,
            ))
            .expect("crop");
        frames.push(captured(img, c, s.vblank(seq), seq));
    }
    s.analysis().with_frames(vec![capture_of(frames)])
}

#[test]
fn an_animated_scroll_does_not_stall() {
    // The content follows the input through a filter: it starts slowly, lags, and comes to rest
    // after the input ends. Its first and last frames move less than a pixel or not at all.
    let tall = texture(160, 4000, 3);
    let shown = std::cell::Cell::new(0.0f64);
    let r = ScrollSmoothness::default().analyze(&played(&tall, 60, |_, consumed| {
        shown.set(shown.get() + (f64::from(consumed) - shown.get()) * 0.12);
        shown.get().round() as i32
    }));
    assert_eq!(r.stalled_frames, 0, "{:?}", stalled(&r));
    assert_eq!(r.stalls, 0);
    assert!(
        r.relative_error.unwrap() > 0.2,
        "the content lags the input"
    );
}

#[test]
fn a_repeated_frame_inside_a_scroll_stalls() {
    // Frames 30 to 32 repeat frame 29 and frame 33 catches up.
    let tall = texture(160, 4000, 3);
    let held = std::cell::Cell::new(0);
    let r = ScrollSmoothness::default().analyze(&played(&tall, 100, |seq, consumed| {
        if !(30..=32).contains(&seq) {
            held.set(consumed);
        }
        held.get()
    }));
    assert_eq!(r.stalled_frames, 3, "{:?}", stalled(&r));
    assert_eq!(r.missed_vblanks, 0);
    assert_eq!(r.stalls, 3);
}

#[test]
fn the_rests_of_a_stepped_scroll_do_not_stall() {
    // One event every tenth of a second, each shown at once: the frames between repeat the
    // picture, and no input asks for more.
    let tall = texture(160, 4000, 3);
    let mut s = Synth::new();
    let mut sent = Vec::new();
    for k in 0..8u64 {
        let t = crate::testing::at_ms(100.0 + k as f64 * 100.0);
        s.input(
            t,
            InputKind::PointerAxis,
            Some((80.0, 100.0)),
            "wheel dx=0 dy=10 v120=(0,80)",
        );
        sent.push(t);
    }
    let mut frames = Vec::new();
    for seq in 1..=120u64 {
        let commit_t = s.vblank(seq - 1) + Nanos::from_millis(5);
        let offset = sent.iter().filter(|t| **t <= commit_t).count() as i32 * 10;
        let c = s.frame(commit_t, seq);
        let img = tall
            .crop(fakeland_types::Rect::new(0, offset, 160, 200))
            .expect("crop");
        frames.push(captured(img, c, s.vblank(seq), seq));
    }
    let r =
        ScrollSmoothness::default().analyze(&s.analysis().with_frames(vec![capture_of(frames)]));
    assert_eq!(r.stalled_frames, 0, "{:?}", stalled(&r));
}

#[test]
fn a_compressed_list_is_followed_at_its_gain() {
    // Thirteen content pixels per scroll pixel: 260 px per refresh over 32 px rows.
    let tall = rows(160, 40_000);
    let mut s = Synth::new();
    let mut sent = Vec::new();
    for k in 0..100u64 {
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
    for seq in 1..=120u64 {
        let commit_t = s.vblank(seq - 1) + Nanos::from_millis(5);
        let offset = sent.iter().filter(|t| **t <= commit_t).count() as i32 * 130;
        let c = s.frame(commit_t, seq);
        let img = tall
            .crop(fakeland_types::Rect::new(0, offset, 160, 600))
            .expect("crop");
        frames.push(captured(img, c, s.vblank(seq), seq));
    }
    let r =
        ScrollSmoothness::default().analyze(&s.analysis().with_frames(vec![capture_of(frames)]));
    assert!((r.content_gain - 13.0).abs() < 0.1, "{}", r.content_gain);
    assert_eq!(r.stalled_frames, 0, "{:?}", stalled(&r));
    assert_eq!(r.rejected_steps, 0, "{:?}", r.rejections);
    assert!(r.relative_error.unwrap() < 0.02, "{:?}", r.relative_error);
}

#[test]
fn a_step_past_the_view_is_rejected_not_stalled() {
    // The content moves three frame heights per refresh: nothing of a frame is left in the next.
    let tall = rows(160, 80_000);
    let r = ScrollSmoothness {
        content_factor: 30.0,
        ..ScrollSmoothness::default()
    }
    .analyze(&played(&tall, 100, |_, consumed| consumed * 30));
    assert_eq!(r.stalled_frames, 0, "{:?}", stalled(&r));
    assert!(r.rejected_steps > 40, "{}", r.rejected_steps);
    assert!(
        r.rejections.contains_key("beyond_view"),
        "{:?}",
        r.rejections
    );
}

/// The indexes of the stalled steps of a report.
fn stalled(r: &ScrollReport) -> Vec<usize> {
    (0..r.steps.len()).filter(|&i| r.steps[i].stalled).collect()
}

#[test]
fn content_that_moves_in_whole_rows_is_followed() {
    // The content moves 48 px per refresh on average but only shows whole 32 px rows, so its
    // steps are one row or two: one period apart.
    let tall = rows(160, 12_000);
    let r = ScrollSmoothness::default().analyze(&played(&tall, 100, |_, consumed| {
        (f64::from(consumed) * 2.4 / 32.0).round() as i32 * 32
    }));
    assert_eq!(r.rejected_steps, 0, "{:?}", r.rejections);
    assert!(
        (r.measured_total_px - 2400.0).abs() < 40.0,
        "{}",
        r.measured_total_px
    );
    let off_row = r
        .steps
        .iter()
        .filter_map(|s| s.measured_px)
        .filter(|m| (m / 32.0 - (m / 32.0).round()).abs() > 0.05)
        .count();
    assert_eq!(off_row, 0);
    assert_eq!(r.stalled_frames, 0, "{:?}", stalled(&r));
}
