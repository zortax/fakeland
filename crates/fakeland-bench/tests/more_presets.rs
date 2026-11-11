//! The remaining presets against the reference client: each runs with its defaults where the
//! client supports it, and its main numbers match what the client does. Run with
//! `cargo test -p fakeland-bench -- --ignored` after `cargo build -p fakeland-testclient`.

mod support;

use fakeland_bench::BenchReport;
use fakeland_bench::presets::{PresetParams, by_name};
use serde_json::json;

fn run(name: &str, app_id: &str, params: PresetParams, s: &fakeland::Session) -> BenchReport {
    let params = params.set("target", json!({"type": "app_id", "app_id": app_id}));
    let report = by_name(name)
        .unwrap()
        .bench(&params)
        .expect("bench")
        .blocking_run(s)
        .expect("run")
        .into_report();
    support::save(&report, name);
    assert!(
        report.iterations.iter().all(|i| i.ok),
        "{name}: {:?}",
        report.iterations
    );
    report
}

fn mean(r: &BenchReport, section: &str, metric: &str) -> f64 {
    r.metric(section, metric, None)
        .unwrap_or_else(|| panic!("{section}.{metric}"))
        .mean
}

#[test]
#[ignore = "runs the reference client"]
fn typing_and_key_latency() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _) = support::client(
        &s,
        "bench.type",
        &["--scene", "type", "--mode", "on-demand"],
    );
    let r = run(
        "typing",
        "bench.type",
        PresetParams::new()
            .set("text", "hello world")
            .set("wpm", 200)
            .set("iterations", 2),
        &s,
    );
    let p50 = mean(&r, "typing", "latency_p50");
    assert!(p50 > 1.0 && p50 < 40.0, "{p50}");
    assert_eq!(mean(&r, "typing", "unpresented"), 0.0);
    let k = run(
        "key-latency",
        "bench.type",
        PresetParams::new().set("repeats", 8).set("iterations", 2),
        &s,
    );
    assert_eq!(mean(&k, "latency", "timeouts"), 0.0);
    assert!(mean(&k, "latency", "input_to_commit_p50") < 10.0);
}

#[test]
#[ignore = "runs the reference client"]
fn click_latency_on_a_toggle() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _) = support::client(
        &s,
        "bench.click",
        &[
            "--scene",
            "click",
            "--mode",
            "on-demand",
            "--render-delay",
            "4ms",
        ],
    );
    let r = run(
        "click-latency",
        "bench.click",
        PresetParams::new().set("repeats", 8).set("iterations", 2),
        &s,
    );
    assert_eq!(mean(&r, "latency", "timeouts"), 0.0);
    let i2c = mean(&r, "latency", "input_to_commit_p50");
    assert!((4.0..6.0).contains(&i2c), "{i2c}");
}

#[test]
#[ignore = "runs the reference client"]
fn resize_storm_and_scale_switch() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _) = support::client(
        &s,
        "bench.resize",
        &["--scene", "resize", "--mode", "on-demand"],
    );
    let r = run(
        "resize-storm",
        "bench.resize",
        PresetParams::new()
            .set("iterations", 2)
            .set("duration", "500ms"),
        &s,
    );
    let to_commit = mean(&r, "resize", "configure_to_commit_p50");
    assert!(to_commit < 20.0, "{to_commit}");
    assert_eq!(mean(&r, "resize", "mismatched_frames"), 0.0);
    let sc = run(
        "scale-switch",
        "bench.resize",
        PresetParams::new()
            .set("scales", [1.5, 1.0])
            .set("iterations", 2)
            .set("wait", "500ms"),
        &s,
    );
    assert_eq!(mean(&sc, "buffers", "upscaled_frames"), 0.0);
    assert_eq!(mean(&sc, "buffers", "oversampled_frames"), 0.0);
    assert!(mean(&sc, "resize", "configure_to_commit_p50") < 50.0);
    let out = s.blocking_outputs().unwrap();
    assert_eq!(out[0].scale, 1.0, "the runner restores the scale");
}

#[test]
#[ignore = "runs the reference client"]
fn throughput_unthrottles_the_output() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _) = support::client(
        &s,
        "bench.fast",
        &[
            "--scene",
            "animate",
            "--mode",
            "unthrottled",
            "--buffers",
            "4",
        ],
    );
    let r = run(
        "throughput",
        "bench.fast",
        PresetParams::new()
            .set("iterations", 2)
            .set("duration", "1s"),
        &s,
    );
    let fps = mean(&r, "throughput", "commit_fps");
    eprintln!("unthrottled commit fps {fps}");
    assert!(fps > 200.0, "{fps}");
    let timing = s.blocking_outputs().unwrap()[0].timing;
    assert_eq!(
        timing.mode,
        fakeland::TimingMode::fixed(fakeland_types::Hz(60.0))
    );
}

#[test]
#[ignore = "runs the reference client"]
fn fling_drag_and_popup() {
    let _serial = support::serial();
    let s = support::session();
    let (app, _) = support::client(
        &s,
        "bench.fling",
        &["--scene", "scroll", "--mode", "callback"],
    );
    let f = run(
        "fling",
        "bench.fling",
        PresetParams::new()
            .set("roi", json!({"x": 440, "y": 160, "w": 400, "h": 400}))
            .set("downscale", 1)
            .set("iterations", 2),
        &s,
    );
    let tracking = mean(&f, "scroll_smoothness", "tracking_ratio");
    assert!((tracking - 1.0).abs() < 0.05, "{tracking}");
    let _ = app.kill();

    let (app, _) = support::client(
        &s,
        "bench.drag",
        &["--scene", "hover", "--pointer-marker", "--mode", "callback"],
    );
    let d = run(
        "drag-follow",
        "bench.drag",
        PresetParams::new()
            .set("roi", json!({"x": 300, "y": 260, "w": 680, "h": 200}))
            .set("downscale", 1)
            .set("iterations", 2),
        &s,
    );
    eprintln!(
        "drag lag p50 {:?}",
        d.metric("drag_follow", "lag_ms_p50", None).map(|m| m.mean)
    );
    assert!(d.section("drag_follow").is_some());
    let _ = app.kill();

    let (_app, _) = support::client(
        &s,
        "bench.popup",
        &["--scene", "hover", "--mode", "on-demand"],
    );
    let p = run(
        "popup-open",
        "bench.popup",
        PresetParams::new()
            .set("iterations", 2)
            .set("open_for", "200ms"),
        &s,
    );
    assert_eq!(
        mean(&p, "popup_open", "opened"),
        0.0,
        "the reference client opens no popups"
    );
}
