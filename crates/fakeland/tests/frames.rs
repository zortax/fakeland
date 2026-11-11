//! Per-commit frame capture against the reference client: every presented frame of a scroll glide
//! is captured, and the content displacement that motion estimation finds between frames matches
//! the scroll offsets the client reports. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use std::collections::HashMap;

use fakeland::scenario::{Position, Scenario, ScrollGlide, ScrollSource};
use fakeland::{CaptureSpec, Event, Fakeland, LaunchSpec, OutputConfig, RunOptions, WindowRef};
use fakeland_trace::AppMarkKind;
use fakeland_types::{CommitId, Dur, Hz, Nanos, Rect};
use fakeland_vision::{Axis, estimate_scroll_1d};

#[test]
#[ignore = "runs the reference client"]
fn presented_frames_follow_the_scroll_offsets() {
    let _serial = support::serial();
    let log_file = support::log_path("frames-scroll");
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()
        .expect("start");
    let app = s
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            "scroll",
            "--mode",
            "callback",
            "--app-id",
            "frames",
            "--exit-after",
            "30",
            "--log-json",
            log_file.to_str().unwrap(),
        ]))
        .expect("launch");
    let w = s
        .blocking_wait_window(WindowRef::app_id("frames"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let me = WindowRef::id(w.id);
    s.blocking_wait_frames(me.clone(), 5, Nanos::from_secs(5))
        .expect("frames");
    let scenario = Scenario::builder("scroll")
        .scroll_glide(
            ScrollGlide::constant(600.0, Dur::from_millis(800))
                .at(Position::center(me.clone()))
                .source(ScrollSource::finger(true)),
        )
        .sleep(Dur::from_millis(100))
        .build()
        .expect("scenario");
    let roi = Rect::new(440, 160, 400, 400);
    let options = RunOptions {
        frames: vec![
            CaptureSpec::every_presented(me.clone()).with_roi(roi),
            CaptureSpec::every_commit(me.clone())
                .with_roi(roi)
                .with_downscale(2),
        ],
    };
    let result = s
        .blocking_run_with(scenario, options)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    let presented = &result.frames[0];
    let commits = &result.frames[1];
    eprintln!(
        "captured {} presented frames ({} dropped, {:.2} ms total) and {} commits",
        presented.frames.len(),
        presented.dropped,
        presented.cost.as_millis_f64(),
        commits.frames.len()
    );
    assert!(presented.frames.len() >= 40);
    assert!(commits.frames.iter().all(|f| f.image.width() == 200));

    let frame_of: HashMap<CommitId, u64> = result
        .trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::AppMark {
                commit: Some(c),
                mark: AppMarkKind::Frame { frame_id },
                ..
            } => Some((*c, *frame_id)),
            _ => None,
        })
        .collect();
    let log = support::read_log(&log_file);
    let offset_of: HashMap<u64, f64> = support::log_events(&log, "commit")
        .iter()
        .filter_map(|c| Some((c["frame_id"].as_u64()?, c["state"]["scroll"][1].as_f64()?)))
        .collect();
    let mut errors = Vec::new();
    let mut moved = 0;
    for pair in presented.frames.windows(2) {
        let (Some(a), Some(b)) = (
            frame_of.get(&pair[0].commit).and_then(|f| offset_of.get(f)),
            frame_of.get(&pair[1].commit).and_then(|f| offset_of.get(f)),
        ) else {
            continue;
        };
        let expected = b - a;
        if expected.abs() < 0.5 || expected.abs() > 60.0 {
            continue;
        }
        let Some(m) = estimate_scroll_1d(&pair[0].image, &pair[1].image, None, Axis::Vertical, 64)
        else {
            continue;
        };
        moved += 1;
        errors.push(((m.dy + expected) * 1e6) as i64);
    }
    let (min, _, max) = support::summarize("scroll displacement error in px", &errors);
    eprintln!("compared {moved} frame pairs");
    assert!(moved >= 30, "{moved}");
    assert!(min > -0.75 && max < 0.75, "{min} {max}");
}
