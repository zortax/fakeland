//! Presets against the reference client, whose injected behavior gives known answers. Run with
//! `cargo test -p fakeland-bench -- --ignored` after `cargo build -p fakeland-testclient`.

mod support;

use fakeland_bench::analyzers::{FramePacing, IdleEfficiency, ScrollSmoothness, Startup};
use fakeland_bench::presets::{PresetParams, by_name};
use fakeland_bench::{Analyzer, BenchRun};
use serde_json::json;

fn preset(name: &str, params: PresetParams, session: &fakeland::Session) -> BenchRun {
    let bench = by_name(name).unwrap().bench(&params).expect("bench");
    let run = bench.blocking_run(session).expect("run");
    support::save(run.report_ref(), name);
    run
}

#[test]
#[ignore = "runs the reference client"]
fn pacing_finds_exactly_the_jank_frames() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _w) = support::client(
        &s,
        "bench.pacing",
        &[
            "--scene",
            "animate",
            "--mode",
            "callback",
            "--render-delay",
            "7ms",
            "--jank-every",
            "10",
            "--jank",
            "25ms",
        ],
    );
    let params = PresetParams::new()
        .set(
            "target",
            json!({"type": "app_id", "app_id": "bench.pacing"}),
        )
        .set("duration", "3s");
    let run = preset("pacing", params, &s);
    let report = run.report();
    assert!(report.is_valid(), "{:?}", report.validity);
    let input = run.measured().next().unwrap().input.as_ref().unwrap();
    let r = FramePacing::default().analyze(input);
    let ids: Vec<u64> = r.jank.frames.iter().filter_map(|f| f.frame_id).collect();
    eprintln!("jank frame ids {ids:?}, presented {}", r.presented);
    let presented: Vec<u64> = input
        .timeline
        .presented(input.main_surface().unwrap())
        .filter_map(|f| f.marks.frame_id)
        .collect();
    let (first, last) = (presented[0], *presented.last().unwrap());
    let expected: Vec<u64> = (first + 1..=last).filter(|i| i % 10 == 0).collect();
    assert_eq!(ids, expected, "every jank frame and no other frame");
    assert!(r.jank.count >= 15);
    let fps = report.metric("frame_pacing", "fps", None).unwrap().mean;
    assert!(fps > 45.0 && fps < 60.5, "{fps}");
    assert!(
        report
            .metric("frame_pacing", "missed_vblanks", None)
            .unwrap()
            .mean
            >= 15.0
    );
}

#[test]
#[ignore = "runs the reference client"]
fn hover_latency_follows_the_render_delay() {
    let _serial = support::serial();
    let s = support::session();
    let (_app, _w) = support::client(
        &s,
        "bench.hover",
        &[
            "--scene",
            "hover",
            "--mode",
            "on-demand",
            "--render-delay",
            "7ms",
        ],
    );
    let params = PresetParams::new()
        .set("target", json!({"type": "app_id", "app_id": "bench.hover"}))
        .set("position", json!({"x": 200, "y": 175}))
        .set("away", json!({"x": 20, "y": 20}))
        .set("roi", json!({"x": 100, "y": 100, "w": 200, "h": 150}))
        .set("repeats", 12)
        .set("iterations", 3);
    let run = preset("hover-latency", params, &s);
    let report = run.report();
    let m = |n: &str| report.metric("latency", n, None).unwrap().mean;
    eprintln!(
        "input->commit p50 {:.3} ms, total p50 {:.3} ms",
        m("input_to_commit_p50"),
        m("total_p50")
    );
    assert_eq!(m("timeouts"), 0.0);
    assert!((7.0..8.5).contains(&m("input_to_commit_p50")));
    assert!(m("total_p50") > 8.0 && m("total_p50") < 7.0 + 1000.0 / 60.0 + 3.0);
    let section = report.section("latency").unwrap();
    assert_eq!(section.pooled[0].samples, Some(36));
    let ci = report.metric("latency", "total_p50", None).unwrap().ci;
    assert!(ci.is_some(), "three iterations give an interval");
    assert!(report.is_valid(), "{:?}", report.validity);
}

fn scroll(s: &fakeland::Session, app_id: &str) -> BenchRun {
    let params = PresetParams::new()
        .set("target", json!({"type": "app_id", "app_id": app_id}))
        .set("roi", json!({"x": 440, "y": 160, "w": 400, "h": 400}))
        .set("velocity", 1200.0)
        .set("iterations", 2);
    preset("scroll-glide", params, s)
}

#[test]
#[ignore = "runs the reference client"]
fn scroll_glide_is_smooth_and_shows_injected_stalls() {
    let _serial = support::serial();
    let s = support::session();
    let (app, _w) = support::client(
        &s,
        "bench.scroll",
        &["--scene", "scroll", "--mode", "callback"],
    );
    let smooth = scroll(&s, "bench.scroll");
    let _ = app.kill();
    let input = smooth.measured().next().unwrap().input.as_ref().unwrap();
    let r = ScrollSmoothness::default().analyze(input);
    eprintln!(
        "smooth: score {:?}, stalls {}, double {}, rel err {:?}, tracking {:?}, start {:?} ms, {} active, {} rejected {:?}",
        r.smoothness_score,
        r.stalls,
        r.double_steps,
        r.relative_error,
        r.tracking_ratio,
        r.start_latency_ms,
        r.active_steps,
        r.rejected_steps,
        r.rejections
    );
    assert!(r.active_steps >= 50);
    assert!(r.rejected_steps * 10 <= r.active_steps);
    assert!(r.stalls <= 1, "{}", r.stalls);
    assert!(r.smoothness_score.unwrap() > 90.0);
    assert!(r.relative_error.unwrap() < 0.1);
    assert!((r.tracking_ratio.unwrap() - 1.0).abs() < 0.05);

    let (_app, _w) = support::client(
        &s,
        "bench.stall",
        &[
            "--scene",
            "scroll",
            "--mode",
            "callback",
            "--jank-every",
            "12",
            "--jank",
            "40ms",
        ],
    );
    let stalled = scroll(&s, "bench.stall");
    let input = stalled.measured().next().unwrap().input.as_ref().unwrap();
    let j = ScrollSmoothness::default().analyze(input);
    eprintln!(
        "stalled: score {:?}, stalls {}, missed {}, double {}",
        j.smoothness_score, j.stalls, j.missed_vblanks, j.double_steps
    );
    for (i, st) in j.steps.iter().enumerate() {
        if st.vblanks_missed > 0
            || j.steps
                .get(i.wrapping_sub(1))
                .is_some_and(|p| p.vblanks_missed > 0)
        {
            eprintln!(
                "step {i}: missed {} expected {:.1} measured {:?} active {}",
                st.vblanks_missed, st.expected_px, st.measured_px, st.active
            );
        }
    }
    assert!(j.missed_vblanks >= 4, "{}", j.missed_vblanks);
    assert!(j.double_steps >= 2, "{}", j.double_steps);
    assert!(j.smoothness_score.unwrap() < r.smoothness_score.unwrap() - 10.0);
}

#[test]
#[ignore = "runs the reference client"]
fn startup_includes_the_startup_delay() {
    let _serial = support::serial();
    let s = support::session();
    let params = PresetParams::new()
        .set(
            "command",
            [
                support::testclient(),
                "--startup-delay".into(),
                "300ms".into(),
                "--scene".into(),
                "hover".into(),
                "--hover-rect".into(),
                "0,0,4000,4000".into(),
                "--app-id".into(),
                "bench.startup".into(),
                "--exit-after".into(),
                "30".into(),
            ],
        )
        .set("iterations", 2)
        .set("warmup", 0)
        .set("stable_frames", 3);
    let run = preset("startup", params, &s);
    let report = run.report();
    assert!(
        report.iterations.iter().all(|i| i.ok),
        "{:?}",
        report.iterations
    );
    for it in run.measured() {
        let r = Startup::default().analyze(it.input.as_ref().unwrap());
        eprintln!("{r:?}");
        let connected = r.connected_ms.unwrap();
        assert!((300.0..1500.0).contains(&connected), "{connected}");
        let present = r.first_present_ms.unwrap();
        assert!(present > r.first_buffer_ms.unwrap(), "{r:?}");
        assert!(r.toplevel_ms.unwrap() <= present + 1000.0 / 60.0, "{r:?}");
        assert!(r.visually_complete_ms.unwrap() >= present);
        assert!(r.responsive_ms.unwrap() > r.visually_complete_ms.unwrap());
        assert!(r.first_input_latency_ms.unwrap() < 50.0);
    }
    assert!(report.metric("startup", "connected", None).unwrap().min >= 300.0);
    assert!(
        s.blocking_windows().unwrap().is_empty(),
        "the app ends after each iteration"
    );
}

#[test]
#[ignore = "runs the reference client"]
fn idle_finds_wasted_frames() {
    let _serial = support::serial();
    let s = support::session();
    let target = |id: &str| json!({"type": "app_id", "app_id": id});
    let (idle_app, _) = support::client(&s, "bench.idle", &["--scene", "idle"]);
    let run = preset(
        "idle",
        PresetParams::new()
            .set("target", target("bench.idle"))
            .set("duration", "2s"),
        &s,
    );
    let quiet =
        IdleEfficiency::default().analyze(run.measured().next().unwrap().input.as_ref().unwrap());
    eprintln!("idle: {quiet:?}");
    assert_eq!(quiet.wasted_commits, 0);
    assert!(quiet.commits_per_s < 1.0);
    let _ = idle_app.kill();

    let (_busy, _) = support::client(
        &s,
        "bench.busy",
        &[
            "--scene", "animate", "--speed", "0", "--mode", "callback", "--damage", "full",
        ],
    );
    let run = preset(
        "idle",
        PresetParams::new()
            .set("target", target("bench.busy"))
            .set("duration", "2s"),
        &s,
    );
    let busy =
        IdleEfficiency::default().analyze(run.measured().next().unwrap().input.as_ref().unwrap());
    eprintln!(
        "busy: commits {} wasted {} unchanged {:?} damage {:?}",
        busy.buffer_commits, busy.wasted_commits, busy.unchanged_commits, busy.damage
    );
    assert!(busy.buffer_commits >= 100);
    assert!(busy.wasted_fraction > 0.9, "{}", busy.wasted_fraction);
    let damage = busy.damage.unwrap();
    assert!(damage.unchanged_damage_fraction.unwrap().p50 > 0.99);
    assert_eq!(damage.under_damage_pixels, 0);
}
