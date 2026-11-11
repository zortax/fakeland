//! Scenarios, triggers and reactors against the reference client: vblank phase anchoring, resize
//! storms, compositor-side ping-pong loops, waits, captures, cancel and errors. Run with
//! `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::scenario::{
    Action, Anchor, Button, PixelCondition, PointerAction, Position, Primitive, Rgba, Roi,
    Scenario, Step, Trigger, TriggerEvent, Wait, WindowAction,
};
use fakeland::{
    CaptureTarget, Event, Fakeland, LaunchSpec, OutputConfig, Reactor, ReactorCx, Record,
    RunErrorKind, RunOptions, Session, WindowRef,
};
use fakeland_trace::AppMarkKind;
use fakeland_types::{
    CommitId, Dur, Hz, InputEventId, Logical, Nanos, Rect, Size, Timestamp, WindowId,
};

fn session(spin: Nanos) -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .timer_spin(spin)
        .start()
        .expect("start")
}

fn launch(session: &Session, args: &[&str], app_id: &str, log: &std::path::Path) -> WindowId {
    let mut all = vec![
        "--app-id",
        app_id,
        "--exit-after",
        "60",
        "--log-json",
        log.to_str().unwrap(),
    ];
    all.extend_from_slice(args);
    let app = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args(all))
        .expect("launch");
    let w = session
        .blocking_wait_window(WindowRef::app_id(app_id), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_settled(
            WindowRef::id(w.id),
            Nanos::from_millis(150),
            Nanos::from_secs(5),
        )
        .expect("settled");
    w.id
}

fn vblank_ideals(trace: &fakeland::Trace) -> Vec<u64> {
    trace
        .records
        .iter()
        .filter_map(|r| match r.event {
            Event::Vblank { ideal, .. } => Some(ideal.0),
            _ => None,
        })
        .collect()
}

fn phase_errors(spin: Nanos) -> Vec<i64> {
    let log = support::log_path("vblank-anchor");
    let s = session(spin);
    let w = launch(&s, &["--scene", "hover"], "anchor", &log);
    let me = WindowRef::id(w);
    let phase = Dur::from_millis(4);
    let step = |x: f64| {
        Step::new(PointerAction::move_to(Position::window(
            me.clone(),
            x,
            10.0,
        )))
        .anchor(Anchor::NextVblank {
            output: None,
            phase,
        })
    };
    let scenario = Scenario {
        name: "anchor".into(),
        steps: vec![Step::new(Action::Repeat {
            count: 60,
            steps: vec![step(10.0), step(30.0)],
        })],
        ..Scenario::default()
    };
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.steps.len(), 121);
    let ideals = vblank_ideals(&s.blocking_trace_snapshot().expect("trace"));
    result
        .trace
        .records
        .iter()
        .filter_map(|r| match r.event {
            Event::InputSent { .. } => Some(r.t.0),
            _ => None,
        })
        .map(|t| {
            let i = ideals.partition_point(|v| *v <= t);
            let base = ideals[i - 1];
            (t - base) as i64 - phase.0.as_nanos()
        })
        .collect()
}

#[test]
#[ignore = "runs the reference client"]
fn next_vblank_anchors_land_on_their_phase() {
    let _serial = support::serial();
    let errors = phase_errors(Nanos::ZERO);
    let (_, med, max) = support::summarize("vblank+4ms anchor error, no spin", &errors);
    assert_eq!(errors.len(), 120);
    assert!(med < 0.3 && max < 2.0, "median {med} max {max}");
    let errors = phase_errors(Nanos::from_micros(300));
    let (_, med, max) = support::summarize("vblank+4ms anchor error, 300 us spin", &errors);
    assert!(med < 0.3 && max < 2.0, "median {med} max {max}");
}

#[test]
#[ignore = "runs the reference client"]
fn resize_storms_get_acked() {
    let _serial = support::serial();
    let log_file = support::log_path("resize-storm");
    let s = session(Nanos::ZERO);
    let w = launch(&s, &["--scene", "resize"], "storm", &log_file);
    let me = WindowRef::id(w);
    let scenario = Scenario::builder("storm")
        .step(Action::Window(WindowAction::ResizeStorm {
            window: me.clone(),
            from: Size::<Logical>::new(400, 300),
            to: Size::<Logical>::new(900, 700),
            duration: Dur::from_millis(500),
            rate_hz: 60.0,
            easing: Default::default(),
        }))
        .wait(Wait::settled(Some(me.clone()), Dur::from_millis(200)))
        .build()
        .expect("scenario");
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    let trace = &result.trace;
    let configures: Vec<(u32, i32, i32)> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::Configure {
                serial,
                width,
                height,
                ..
            } => Some((*serial, *width, *height)),
            _ => None,
        })
        .collect();
    let acks: Vec<u32> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::AckConfigure { serial, .. } => Some(*serial),
            _ => None,
        })
        .collect();
    let commits_with_serial = trace
        .records
        .iter()
        .filter(|r| {
            matches!(
                &r.event,
                Event::Commit {
                    configure_serial: Some(_),
                    ..
                }
            )
        })
        .count();
    eprintln!(
        "resize storm: {} configures, {} acks, {} commits carry a serial",
        configures.len(),
        acks.len(),
        commits_with_serial
    );
    assert!(configures.len() >= 30);
    let last = configures.last().unwrap();
    assert_eq!((last.1, last.2), (900, 700));
    assert_eq!(
        acks.last(),
        Some(&last.0),
        "the client acks the last configure"
    );
    assert!(commits_with_serial >= acks.len().saturating_sub(1));
    let log = support::read_log(&log_file);
    let size = support::log_events(&log, "commit").last().unwrap()["size"].clone();
    assert_eq!(size, serde_json::json!([900, 700]));
}

#[test]
#[ignore = "runs the reference client"]
fn a_trigger_loop_clicks_on_every_presented_change() {
    let _serial = support::serial();
    let log_file = support::log_path("trigger-loop");
    let s = session(Nanos::ZERO);
    let w = launch(
        &s,
        &[
            "--scene",
            "click",
            "--mode",
            "on-demand",
            "--render-delay",
            "3ms",
        ],
        "trigger",
        &log_file,
    );
    let me = WindowRef::id(w);
    let click = Action::Pointer(PointerAction::Click {
        at: Some(Position::center(me.clone())),
        button: Button::Left,
        count: 1,
        interval: Dur::from_millis(120),
        hold: Dur::from_millis(1),
    });
    let scenario = Scenario::builder("ping-pong")
        .step(click.clone())
        .sleep(Dur::from_millis(1500))
        .trigger(
            Trigger::on(
                TriggerEvent::RoiChanged {
                    roi: Roi::window_rect(me.clone(), Rect::new(540, 285, 200, 150)),
                    threshold: 0.001,
                },
                [click],
            )
            .limit(10),
        )
        .build()
        .expect("scenario");
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.triggers.len(), 10, "{:?}", result.triggers);
    assert!(result.triggers.iter().all(|t| t.error.is_none()));
    let react: Vec<i64> = result
        .triggers
        .iter()
        .map(|t| t.fired.since(t.event_time).as_nanos())
        .collect();
    support::summarize("trigger reaction (presented record -> actions)", &react);
    let costs: Vec<i64> = result
        .trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::Capture { cost, .. } => Some(cost.as_nanos()),
            _ => None,
        })
        .collect();
    support::summarize("trigger region capture cost", &costs);
    let log = support::read_log(&log_file);
    let presses = support::log_events(&log, "commit")
        .last()
        .and_then(|c| c["state"]["presses"].as_u64())
        .unwrap_or(0);
    assert_eq!(presses, 11, "the first click and ten trigger clicks");
}

/// A reactor that moves the pointer in and out of the hover target. Each move follows the
/// presentation of the commit that handled the previous move, and the reactor reports the
/// latency from send to presentation.
struct PingPong {
    window: WindowId,
    inside: bool,
    pending: Option<(InputEventId, Timestamp)>,
    handled: Option<CommitId>,
    left: u32,
}

impl PingPong {
    fn send(&mut self, cx: &mut ReactorCx<'_, (Timestamp, Timestamp)>) {
        self.inside = !self.inside;
        let (x, y) = if self.inside {
            (200.0, 175.0)
        } else {
            (20.0, 20.0)
        };
        let sent = cx
            .inject(&[
                Primitive::PointerMotion {
                    pos: Position::window(WindowRef::id(self.window), x, y),
                },
                Primitive::PointerFrame,
            ])
            .expect("inject");
        self.pending = sent.first().map(|s| (s.id, s.time));
        self.handled = None;
    }
}

impl Reactor for PingPong {
    type Value = (Timestamp, Timestamp);

    fn on_start(&mut self, cx: &mut ReactorCx<'_, Self::Value>) {
        self.send(cx);
    }

    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, Self::Value>) {
        match &record.event {
            Event::AppMark {
                commit: Some(c),
                mark: AppMarkKind::InputHandled { input },
                ..
            } if self.pending.is_some_and(|(id, _)| id == *input) => self.handled = Some(*c),
            Event::Presented { commit, time, .. } if Some(*commit) == self.handled => {
                if let Some((_, sent)) = self.pending.take() {
                    cx.emit((sent, *time));
                }
                self.left -= 1;
                if self.left == 0 {
                    cx.finish();
                } else {
                    self.send(cx);
                }
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "runs the reference client"]
fn a_reactor_runs_a_ping_pong_loop() {
    let _serial = support::serial();
    let log_file = support::log_path("reactor");
    let s = session(Nanos::ZERO);
    let w = launch(
        &s,
        &[
            "--scene",
            "hover",
            "--mode",
            "on-demand",
            "--render-delay",
            "4ms",
        ],
        "reactor",
        &log_file,
    );
    let handle = s
        .blocking_install_reactor(PingPong {
            window: w,
            inside: false,
            pending: None,
            handled: None,
            left: 20,
        })
        .expect("install");
    let mut latencies = Vec::new();
    while let Some((sent, presented)) = handle.blocking_recv() {
        latencies.push(presented.since(sent).as_nanos());
    }
    let (min, _, max) = support::summarize("reactor send -> present", &latencies);
    assert_eq!(latencies.len(), 20);
    assert!(min > 4.0 && max < 40.0);
    let log = support::read_log(&log_file);
    let hovers = support::log_events(&log, "commit")
        .iter()
        .filter(|c| c["handled"].as_array().is_some_and(|h| !h.is_empty()))
        .count();
    assert!(hovers >= 20);
}

#[test]
#[ignore = "runs the reference client"]
fn waits_captures_and_run_errors() {
    let _serial = support::serial();
    let log_file = support::log_path("waits");
    let s = session(Nanos::ZERO);
    let w = launch(
        &s,
        &["--scene", "hover", "--mode", "on-demand"],
        "waits",
        &log_file,
    );
    let me = WindowRef::id(w);
    let roi = Roi::window_rect(me.clone(), Rect::new(100, 100, 200, 150));
    let scenario = Scenario::builder("waits")
        .pointer_move(Position::window(me.clone(), 200.0, 175.0))
        .wait(Wait::pixels_match(
            roi.clone(),
            PixelCondition::ColorAt {
                x: 100,
                y: 75,
                rgba: Rgba::rgb(0xe0, 0x60, 0x20),
                tolerance: 8,
            },
        ))
        .wait(Wait::pixels_stable(roi.clone(), 3))
        .label("after")
        .capture(CaptureTarget::Roi { roi: roi.clone() })
        .mark("done")
        .build()
        .expect("scenario");
    let result = s
        .blocking_run(scenario)
        .expect("run")
        .blocking_wait()
        .expect("result");
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.captures.len(), 1);
    let shot = &result.captures[0].screenshot;
    assert_eq!(
        shot.image.pixel(100, 75).map(|p| [p[0], p[1], p[2]]),
        Some([0xe0, 0x60, 0x20])
    );
    let names: Vec<&str> = result
        .trace
        .records
        .iter()
        .map(|r| r.event.name())
        .collect();
    assert!(names.contains(&"scenario_started") && names.contains(&"scenario_finished"));
    assert!(result.steps.iter().all(|t| t.finished >= t.started));
    eprintln!(
        "steps: {:?}",
        result
            .steps
            .iter()
            .map(|t| (
                t.action.clone(),
                t.finished.since(t.started).as_millis_f64()
            ))
            .collect::<Vec<_>>()
    );

    let missing = Scenario::builder("missing")
        .click(Position::center(WindowRef::app_id("nope")))
        .build()
        .expect("scenario");
    let result = s.blocking_run(missing).unwrap().blocking_wait().unwrap();
    assert_eq!(result.error_kind, Some(RunErrorKind::Step));
    assert!(result.error.unwrap().contains("steps[0]"));

    let long = Scenario::builder("long")
        .sleep(Dur::from_secs(30))
        .build()
        .expect("scenario");
    let handle = s.blocking_run(long).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(handle.blocking_cancel().unwrap());
    let result = handle.blocking_wait().unwrap();
    assert_eq!(result.error_kind, Some(RunErrorKind::Cancelled));

    let mut timed = Scenario::builder("timed")
        .sleep(Dur::from_secs(30))
        .build()
        .expect("scenario");
    timed.timeout = Some(Dur::from_millis(200));
    let result = s.blocking_run(timed).unwrap().blocking_wait().unwrap();
    assert_eq!(result.error_kind, Some(RunErrorKind::Timeout));
    assert!(result.duration() < Nanos::from_millis(400));

    let opts = RunOptions {
        frames: vec![fakeland::CaptureSpec::every_presented(me.clone()).with_downscale(4)],
    };
    let moves = Scenario::builder("frames")
        .pointer_move(Position::window(me.clone(), 20.0, 20.0))
        .wait(Wait::frames(1, Some(me.clone())))
        .pointer_move(Position::window(me.clone(), 200.0, 175.0))
        .wait(Wait::frames(1, Some(me.clone())))
        .build()
        .expect("scenario");
    let result = s
        .blocking_run_with(moves, opts)
        .unwrap()
        .blocking_wait()
        .unwrap();
    assert!(result.ok, "{:?}", result.error);
    let frames = &result.frames[0].frames;
    assert!(frames.len() >= 2, "{}", frames.len());
    assert!(
        frames
            .iter()
            .all(|f| f.presented.is_some() && f.image.width() == 320)
    );
}
