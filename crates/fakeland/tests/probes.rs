//! Latency probes against the reference client: hover probes with pixel, damage and app-mark
//! detectors, stage times compared with the client log, and vblank alignment. Run with
//! `cargo test -p fakeland -- --ignored` after `cargo build -p fakeland-testclient`.

mod support;

use std::collections::BTreeSet;

use fakeland::scenario::{Action, Detector, PhaseSpec, PointerAction, Position, ProbeSpec, Roi};
use fakeland::{Event, Fakeland, LaunchSpec, OutputConfig, ProbeReport, Session, WindowRef};
use fakeland_types::{Hz, Nanos, Rect, Timestamp, WindowId};

fn hover_client(log: &std::path::Path) -> LaunchSpec {
    LaunchSpec::new(support::testclient()).args([
        "--scene",
        "hover",
        "--mode",
        "on-demand",
        "--render-delay",
        "7ms",
        "--app-id",
        "probe.hover",
        "--exit-after",
        "60",
        "--log-json",
        log.to_str().unwrap(),
    ])
}

fn start(log: &std::path::Path) -> (Session, fakeland::App, WindowId) {
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()
        .expect("start");
    let app = session.blocking_launch(hover_client(log)).expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id("probe.hover"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_settled(
            WindowRef::id(window.id),
            Nanos::from_millis(200),
            Nanos::from_secs(5),
        )
        .expect("settled");
    (session, app, window.id)
}

fn hover_spec(window: WindowId, detector: Detector, repeat: u32) -> ProbeSpec {
    let me = WindowRef::id(window);
    ProbeSpec::new(
        Action::Pointer(PointerAction::move_to(Position::window(
            me.clone(),
            200.0,
            175.0,
        ))),
        detector,
    )
    .alternate(Action::Pointer(PointerAction::move_to(Position::window(
        me, 20.0, 20.0,
    ))))
    .phase(PhaseSpec::Sweep { steps: 8 })
    .repeat(repeat)
}

fn hover_roi(window: WindowId) -> Roi {
    Roi::window_rect(WindowRef::id(window), Rect::new(100, 100, 200, 150))
}

fn ns(a: Timestamp, b: Timestamp) -> i64 {
    b.0 as i64 - a.0 as i64
}

#[test]
#[ignore = "runs the reference client"]
fn hover_probe_stages_match_the_client() {
    let _serial = support::serial();
    let log_file = support::log_path("hover-probe");
    let (session, _app, window) = start(&log_file);
    let detector = Detector::PixelsChanged {
        roi: hover_roi(window),
        threshold: 0.01,
    };
    let report = session
        .blocking_probe(hover_spec(window, detector, 24))
        .expect("probe");
    let trace = session.blocking_trace_snapshot().expect("trace");
    let log = support::read_log(&log_file);
    assert_eq!(report.timeouts(), 0, "every repeat detects the hover");

    let vblanks: BTreeSet<u64> = trace
        .records
        .iter()
        .filter_map(|r| match r.event {
            Event::Vblank { ideal, .. } => Some(ideal.0),
            _ => None,
        })
        .collect();
    let commits = support::log_events(&log, "commit");
    let received = support::log_events(&log, "input_received");
    let (mut i2c, mut commit_lag, mut delivery, mut render, mut phase_err) =
        (vec![], vec![], vec![], vec![], vec![]);
    for r in &report.results {
        let input = r.inputs[0];
        let sent = r.input_sent.unwrap();
        let committed = r.committed.unwrap();
        let presented = r.presented.unwrap();
        assert!(
            vblanks.contains(&presented.0),
            "repeat {} presents on the vblank grid",
            r.iteration
        );
        assert_eq!(r.presented_commit, r.detected_commit);
        let line = commits
            .iter()
            .find(|c| {
                c["handled"]
                    .as_array()
                    .is_some_and(|h| h.iter().any(|v| v.as_u64() == Some(input.0)))
            })
            .unwrap_or_else(|| panic!("the client handled input {input}"));
        let client_commit = Timestamp(line["t"].as_u64().unwrap());
        let got = received
            .iter()
            .find(|l| l["marks_id"].as_u64() == Some(input.0))
            .expect("received");
        let client_received = Timestamp(got["t"].as_u64().unwrap());
        let begin = Timestamp(line["render_begin"].as_u64().unwrap());
        let end = Timestamp(line["render_end"].as_u64().unwrap());
        commit_lag.push(ns(client_commit, committed));
        delivery.push(ns(sent, client_received));
        render.push(ns(begin, end));
        if r.iteration > 0 {
            i2c.push(ns(sent, committed));
        }
        if let (Some(want), Some(got)) = (r.requested_phase, r.phase_in_frame) {
            phase_err.push(got.as_nanos() - want.as_nanos());
        }
        assert!(
            ns(client_commit, committed) >= 0,
            "the compositor sees the commit after the client sends it"
        );
    }
    support::summarize("probe input->commit (after warmup)", &i2c);
    support::summarize("client commit -> compositor commit", &commit_lag);
    support::summarize("input send -> client receive", &delivery);
    support::summarize("client render span", &render);
    support::summarize("injection phase error", &phase_err);
    let totals: Vec<i64> = report.totals().iter().map(|n| n.as_nanos()).collect();
    support::summarize("probe total", &totals);
    let (_, med, max) = support::summarize("probe input->commit", &i2c);
    assert!(
        (7.0..8.0).contains(&med),
        "input->commit follows the 7 ms render: {med}"
    );
    assert!(
        max < 10.0,
        "input->commit stays near the render delay: {max}"
    );
    let (_, lag_med, _) = support::summarize("lag", &commit_lag);
    assert!(
        lag_med < 0.5,
        "commit receipt is within tens of microseconds: {lag_med}"
    );
    let (_, _, perr_max) = support::summarize("phase", &phase_err);
    assert!(perr_max < 1.0, "the input lands near its phase: {perr_max}");
    let latency_spread = totals.iter().max().unwrap() - totals.iter().min().unwrap();
    assert!(
        latency_spread > 10_000_000,
        "a phase sweep spreads the total over most of a frame"
    );
}

fn detected(report: &ProbeReport) -> Vec<Option<fakeland_types::CommitId>> {
    report.results.iter().map(|r| r.detected_commit).collect()
}

#[test]
#[ignore = "runs the reference client"]
fn detectors_agree_on_the_response_commit() {
    let _serial = support::serial();
    let log_file = support::log_path("hover-detectors");
    let (session, _app, window) = start(&log_file);
    let damage = session
        .blocking_probe(hover_spec(
            window,
            Detector::AnyDamage {
                roi: hover_roi(window),
            },
            6,
        ))
        .expect("damage probe");
    let marks = session
        .blocking_probe(hover_spec(window, Detector::AppMark { name: None }, 6))
        .expect("mark probe");
    let pixels = session
        .blocking_probe(hover_spec(
            window,
            Detector::PixelsChanged {
                roi: hover_roi(window),
                threshold: 0.01,
            },
            6,
        ))
        .expect("pixel probe");
    for report in [&damage, &marks, &pixels] {
        assert_eq!(
            report.timeouts(),
            0,
            "{} detects every repeat",
            report.detector
        );
        for r in &report.results {
            let i2c = r.stages.input_to_commit.unwrap().as_millis_f64();
            assert!(i2c > 6.9 && i2c < 12.0, "{}: {i2c}", report.detector);
        }
    }
    assert_eq!(damage.results.len(), 6);
    let trace = session.blocking_trace_snapshot().expect("trace");
    for (a, b) in [(&damage, &marks), (&marks, &pixels)] {
        assert_eq!(detected(a).len(), detected(b).len());
    }
    for r in marks.results.iter() {
        let commit = r.detected_commit.unwrap();
        let handled = trace.records.iter().any(|rec| {
            matches!(&rec.event, Event::AppMark {
                commit: Some(c),
                mark: fakeland_trace::AppMarkKind::InputHandled { input },
                ..
            } if *c == commit && r.inputs.contains(input))
        });
        assert!(
            handled,
            "the mark names the probe input on the detected commit"
        );
    }
    let probe_events = trace
        .records
        .iter()
        .filter(|r| matches!(r.event, Event::ProbeDetected { .. }))
        .count();
    assert!(probe_events >= 18);
}
