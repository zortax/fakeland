//! Device models with the reference client: touch taps and touch glides, touchpad gestures, the
//! pointer report rate and text through text input. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::scenario::{Action, Gesture, Path, PointerGlide, Position, Scenario, TouchAction};
use fakeland::{
    Event, Fakeland, InputConfig, LaunchSpec, OutputConfig, PointerDevice, Session, WindowRef,
};
use fakeland_trace::InputKind;
use fakeland_types::{Dur, Hz, Nanos, WindowId};

fn start(
    input: InputConfig,
    scene: &str,
    app_id: &str,
    log: &std::path::Path,
) -> (Session, WindowId) {
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .input(input)
        .app_marks(true)
        .start()
        .expect("start");
    let app = s
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            scene,
            "--app-id",
            app_id,
            "--exit-after",
            "30",
            "--log-json",
            log.to_str().unwrap(),
        ]))
        .expect("launch");
    let w = s
        .blocking_wait_window(WindowRef::app_id(app_id), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    s.blocking_wait_settled(
        WindowRef::id(w.id),
        Nanos::from_millis(150),
        Nanos::from_secs(5),
    )
    .expect("settled");
    (s, w.id)
}

fn sent_kinds(trace: &fakeland::Trace) -> Vec<InputKind> {
    trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::InputSent { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect()
}

#[test]
#[ignore = "runs the reference client"]
fn touch_taps_and_glides_reach_the_client() {
    let _serial = support::serial();
    let log_file = support::log_path("touch");
    let input = InputConfig {
        touch: true,
        ..InputConfig::default()
    };
    let (s, w) = start(input, "click", "touch", &log_file);
    let me = WindowRef::id(w);
    s.blocking_touch_tap(Position::center(me.clone()))
        .expect("tap");
    let glide = Action::Touch(TouchAction::glide(
        Path::Line {
            from: Position::window(me.clone(), 100.0, 100.0),
            to: Position::window(me.clone(), 300.0, 200.0),
        },
        Dur::from_millis(200),
    ));
    s.blocking_act(glide).expect("glide");
    s.blocking_wait_settled(me, Nanos::from_millis(150), Nanos::from_secs(5))
        .expect("settle");
    let log = support::read_log(&log_file);
    let events: Vec<String> = support::log_events(&log, "input_received")
        .into_iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
        .filter_map(|e| e["type"].as_str().map(str::to_string))
        .filter(|t| t.starts_with("touch"))
        .collect();
    assert_eq!(events.iter().filter(|e| *e == "touch_down").count(), 2);
    assert_eq!(events.iter().filter(|e| *e == "touch_up").count(), 2);
    assert!(events.iter().filter(|e| *e == "touch_motion").count() >= 20);
    let presses = support::log_events(&log, "commit")
        .last()
        .and_then(|c| c["state"]["presses"].as_u64())
        .unwrap_or(0);
    assert_eq!(
        presses, 2,
        "both touch points went down inside the click rect"
    );
}

#[test]
#[ignore = "runs the reference client"]
fn gestures_and_report_rate() {
    let _serial = support::serial();
    let log_file = support::log_path("report-rate");
    let input = InputConfig {
        pointer: PointerDevice {
            report_rate: Some(Hz(125.0)),
            ..PointerDevice::default()
        },
        ..InputConfig::default()
    };
    let (s, w) = start(input, "hover", "rate", &log_file);
    let me = WindowRef::id(w);
    let capture = s.blocking_begin_capture().expect("capture");
    let glide = PointerGlide::new(
        Path::Line {
            from: Position::window(me.clone(), 10.0, 10.0),
            to: Position::window(me.clone(), 610.0, 410.0),
        },
        Dur::from_millis(400),
    )
    .rate(1000.0);
    let scenario = Scenario::builder("rate")
        .glide(glide)
        .sleep(Dur::from_millis(50))
        .gesture(Gesture::pinch(
            Position::center(me.clone()),
            2.0,
            Dur::from_millis(100),
        ))
        .gesture(Gesture::swipe(100.0, 0.0, Dur::from_millis(100)))
        .build()
        .expect("scenario");
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    let trace = s.blocking_end_capture(capture).expect("trace");
    let motions: Vec<(u64, f64, f64)> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::InputSent {
                kind: InputKind::PointerMotion,
                x: Some(x),
                y: Some(y),
                ..
            } => Some((r.t.0, *x, *y)),
            _ => None,
        })
        .collect();
    eprintln!(
        "401 glide motions at 1 kHz became {} reports",
        motions.len()
    );
    assert!((45..=55).contains(&motions.len()), "{}", motions.len());
    let gaps: Vec<i64> = motions
        .windows(2)
        .filter(|p| p[1].0 - p[0].0 < 20_000_000)
        .map(|p| (p[1].0 - p[0].0) as i64)
        .collect();
    let (min, _, _) = support::summarize("report gap", &gaps);
    assert!(min > 7.9, "reports keep the 8 ms period: {min}");
    let geometry = s.blocking_windows().expect("windows")[0].geometry;
    let last = motions[motions.len() - 2];
    assert_eq!(
        (last.1, last.2),
        (geometry.x as f64 + 610.0, geometry.y as f64 + 410.0),
        "the last glide report carries the glide end"
    );
    let center = motions[motions.len() - 1];
    assert_eq!(
        (center.1, center.2),
        (640.0, 360.0),
        "the pinch starts at the center"
    );
    let kinds = sent_kinds(&trace);
    for k in [
        InputKind::PinchBegin,
        InputKind::PinchUpdate,
        InputKind::PinchEnd,
        InputKind::SwipeBegin,
        InputKind::SwipeUpdate,
        InputKind::SwipeEnd,
    ] {
        assert!(kinds.contains(&k), "{k:?}");
    }

    let capture = s.blocking_begin_capture().expect("capture");
    let glide = PointerGlide::new(
        Path::Line {
            from: Position::window(me.clone(), 20.0, 20.0),
            to: Position::window(me.clone(), 420.0, 220.0),
        },
        Dur::from_millis(200),
    )
    .rate(1000.0);
    let outcome = s.blocking_act(glide.into()).expect("glide");
    let trace = s.blocking_end_capture(capture).expect("trace");
    let fakeland::ActionOutcome::Input { sent } = outcome else {
        panic!("a glide returns its input");
    };
    let returned: Vec<_> = sent
        .iter()
        .filter(|e| e.kind == InputKind::PointerMotion)
        .map(|e| e.id)
        .collect();
    let recorded: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::InputSent {
                id,
                kind: InputKind::PointerMotion,
                ..
            } => Some(*id),
            _ => None,
        })
        .collect();
    assert!(recorded.len() < 30, "{} reports", recorded.len());
    assert_eq!(returned, recorded, "the glide returns its deferred reports");
}

#[test]
#[ignore = "runs the reference client"]
fn text_input_commits_reach_the_client() {
    let _serial = support::serial();
    let log_file = support::log_path("commit-text");
    let (s, w) = start(InputConfig::default(), "type", "commit", &log_file);
    s.blocking_click(Position::center(WindowRef::id(w)), Default::default())
        .expect("focus");
    let scenario = Scenario::builder("commit")
        .commit_text("grüße ✓")
        .settle(Dur::from_millis(150))
        .build()
        .expect("scenario");
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    let log = support::read_log(&log_file);
    let texts: Vec<String> = support::log_events(&log, "input_received")
        .into_iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
        .filter(|e| e["type"] == "text")
        .filter_map(|e| e["text"].as_str().map(str::to_string))
        .collect();
    assert_eq!(texts, vec!["grüße ✓".to_string()]);
}
