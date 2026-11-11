//! The runner against the reference client: variants with output restore, the ping-pong reactor
//! driver with and without marks, a custom async driver, and report round trips. Run with
//! `cargo test -p fakeland-bench -- --ignored` after `cargo build -p fakeland-testclient`.

mod support;

use fakeland::reactor::ReactorCx;
use fakeland::scenario::Position;
use fakeland::{Record, WindowRef};
use fakeland_bench::presets::{PresetParams, by_name};
use fakeland_bench::reactors::{PingPong, PingPongLatency};
use fakeland_bench::{
    Bench, BenchReport, DriverOutput, Observation, Thresholds, compare, driver_fn,
};
use fakeland_types::{Dur, Hz, Nanos};
use serde_json::json;

#[test]
#[ignore = "runs the reference client"]
fn refresh_variants_measure_each_rate_and_restore_the_output() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _w) = support::client(
        &s,
        "bench.sweep",
        &["--scene", "animate", "--mode", "callback"],
    );
    let params = PresetParams::new()
        .set("target", json!({"type": "app_id", "app_id": "bench.sweep"}))
        .set("rates", [60.0, 144.0])
        .set("duration", "1500ms");
    let run = by_name("refresh-sweep")
        .unwrap()
        .bench(&params)
        .unwrap()
        .blocking_run(&s)
        .unwrap();
    let report = run.report();
    support::save(&report, "refresh-sweep");
    let fps = |v: &str| report.metric("frame_pacing", "fps", Some(v)).unwrap().mean;
    assert!((fps("60Hz") - 60.0).abs() < 1.0, "{}", fps("60Hz"));
    assert!((fps("144Hz") - 144.0).abs() < 3.0, "{}", fps("144Hz"));
    assert_eq!(report.config.variants, ["60Hz", "144Hz"]);
    let out = s.blocking_outputs().unwrap();
    assert_eq!(
        out[0].mode.refresh,
        Hz(60.0),
        "the runner restores the mode"
    );
    assert_eq!((out[0].mode.width, out[0].mode.height), (1280, 720));
    let back = BenchReport::from_json(&report.to_json_pretty()).unwrap();
    assert_eq!(back, report);
    let same = compare(&report, &back, &Thresholds::default());
    assert_eq!(same.regressions, 0);
    assert_eq!(same.improvements, 0);
}

#[test]
#[ignore = "runs the reference client"]
fn ping_pong_reactor_with_and_without_marks() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _w) = support::client(
        &s,
        "bench.pingpong",
        &[
            "--scene",
            "hover",
            "--mode",
            "on-demand",
            "--render-delay",
            "5ms",
        ],
    );
    let bench = Bench::new("ping-pong")
        .target(WindowRef::app_id("bench.pingpong"))
        .reactor(Dur::from_secs(5), |cx| {
            let w = cx.window();
            PingPong::new(
                Position::window(w.clone(), 200.0, 175.0),
                Position::window(w, 20.0, 20.0),
                10,
            )
        })
        .iterations(3)
        .warmup(1)
        .analyze(PingPongLatency);
    let report = bench.blocking_run(&s).unwrap().into_report();
    support::save(&report, "ping-pong");
    assert!(
        report.iterations.iter().all(|i| i.ok),
        "{:?}",
        report.iterations
    );
    let section = report.section("ping_pong").unwrap();
    assert_eq!(section.samples, Some(30));
    let total = section.metric("total_p50", None).unwrap();
    assert_eq!(total.n, 3);
    assert!(
        total.mean > 5.0 && total.mean < 5.0 + 2.0 * 1000.0 / 60.0,
        "{}",
        total.mean
    );
    assert!(total.ci.is_some());
    let i2c = section.metric("input_to_commit_p50", None).unwrap().mean;
    assert!((5.0..6.5).contains(&i2c), "{i2c}");

    let no_marks = Bench::new("ping-pong-no-marks")
        .target(WindowRef::app_id("bench.pingpong"))
        .reactor(Dur::from_secs(5), |cx| {
            let w = cx.window();
            let surface = cx.target.as_ref().map(|t| t.surface);
            PingPong::new(
                Position::window(w.clone(), 200.0, 175.0),
                Position::window(w, 20.0, 20.0),
                10,
            )
            .without_marks(surface)
        })
        .analyze(PingPongLatency);
    let r = no_marks.blocking_run(&s).unwrap().into_report();
    assert_eq!(r.section("ping_pong").unwrap().samples, Some(10));
    let i2c = r
        .metric("ping_pong", "input_to_commit_p50", None)
        .unwrap()
        .mean;
    assert!((5.0..6.5).contains(&i2c), "{i2c}");
}

#[test]
#[ignore = "runs the reference client"]
fn custom_async_driver_and_reactor_timeout() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _w) = support::client(
        &s,
        "bench.custom",
        &["--scene", "hover", "--mode", "on-demand"],
    );
    let bench = Bench::new("custom")
        .target(WindowRef::app_id("bench.custom"))
        .driver(driver_fn(|session, cx| async move {
            let w = cx.window();
            let (x, y) = if cx.index % 2 == 0 {
                (200.0, 175.0)
            } else {
                (20.0, 20.0)
            };
            let sent = session
                .pointer_move(Position::window(w.clone(), x, y))
                .await?;
            let t = session.wait_frames(w, 1, Nanos::from_secs(2)).await?;
            Ok(DriverOutput {
                observations: vec![
                    Observation::new("sent", sent[0].time),
                    Observation::new("presented", t),
                ],
                ..DriverOutput::default()
            })
        }))
        .iterations(2);
    let run = bench.blocking_run(&s).unwrap();
    for it in run.measured() {
        assert!(it.ok(), "{:?}", it.error);
        let input = it.input.as_ref().unwrap();
        let (a, b) = (
            input.observation("sent").unwrap(),
            input.observation("presented").unwrap(),
        );
        assert!(b > a);
        assert!(input.target.is_some());
        assert!(!input.trace.records.is_empty());
    }
    assert_eq!(run.traces().len(), 2);

    let stuck = Bench::new("stuck")
        .target(WindowRef::app_id("bench.custom"))
        .reactor(Dur::from_millis(300), |_| {
            fakeland::reactor_fn(|_: &Record, _: &mut ReactorCx<'_, u32>| {})
        })
        .iterations(1);
    let report = stuck.blocking_run(&s).unwrap().into_report();
    assert!(!report.iterations[0].ok);
    assert!(!report.is_valid(), "a failed iteration fails the run");
    assert!(
        report.iterations[0]
            .error
            .as_ref()
            .unwrap()
            .contains("ran past")
    );
}
