//! Input-to-photon latency of a hover. A latency probe moves the pointer into the hover target of
//! the reference client and out again, 24 times, with the injection swept over 8 phases of the
//! refresh period. The client renders on demand with a 7 ms render delay. The probe runs once
//! with a pixel detector on the target region and once with the app-mark detector.
//!
//! The probe splits each repeat into stages:
//! - input -> commit: the send of the pointer motion to the commit of the first frame that shows
//!   the hover. It holds event delivery, client handling and the client render.
//! - commit -> ready: the commit to the GPU fence signal. A buffer without a fence is ready at
//!   commit.
//! - ready -> latch: the wait for the latch deadline of the next refresh.
//! - latch -> present: the latch to the modeled presentation on the vblank grid.
//! - total: the send of the input to the presentation.
//!
//! Every time is a compositor timestamp on `CLOCK_MONOTONIC`. The pixel detector reads back the
//! target region of each presented frame and matches the first change. The app-mark detector
//! matches the commit that the client marks with `input_handled` for the injected event.
//!
//! Run with `just example hover_latency`, or `cargo run -p fakeland-examples --example
//! hover_latency` after `cargo build -p fakeland-testclient`. `--hz N` sets the refresh rate
//! (default 60).

use fakeland::scenario::{Action, Detector, PhaseSpec, PointerAction, Position, ProbeSpec, Roi};
use fakeland::{
    Fakeland, LaunchSpec, OutputConfig, ProbeReport, ProbeResult, ProbeStages, WindowRef,
};
use fakeland_examples::{testclient_argv, value};
use fakeland_trace::stats::{Summary, median};
use fakeland_types::{Hz, Nanos, Rect};

const REPEATS: u32 = 24;
const PHASES: u32 = 8;

/// The hover target of the reference client, in window coordinates.
const TARGET: Rect<fakeland_types::Logical> = Rect::new(100, 100, 200, 150);

/// Returns the value after a flag, for example `--hz 144`.
fn probe(window: &WindowRef, detector: Detector) -> ProbeSpec {
    let inside = Position::window(window.clone(), 200.0, 175.0);
    let outside = Position::window(window.clone(), 20.0, 20.0);
    ProbeSpec::new(Action::Pointer(PointerAction::move_to(inside)), detector)
        .alternate(Action::Pointer(PointerAction::move_to(outside)))
        .phase(PhaseSpec::Sweep { steps: PHASES })
        .repeat(REPEATS)
}

type Stage = fn(&ProbeStages) -> Option<Nanos>;

const STAGES: [(&str, Stage); 5] = [
    ("input -> commit", |s| s.input_to_commit),
    ("commit -> ready", |s| s.commit_to_ready),
    ("ready -> latch", |s| s.ready_to_latch),
    ("latch -> present", |s| s.latch_to_present),
    ("total", |s| s.total),
];

/// Formats p50, p95 and max of one stage in milliseconds.
fn cell(report: &ProbeReport, stage: Stage) -> String {
    match Summary::of_millis(report.results.iter().filter_map(|r| stage(&r.stages))) {
        Some(s) => format!("{:>6.2} {:>6.2} {:>6.2}", s.p50, s.p95, s.max),
        None => format!("{:>20}", "-"),
    }
}

/// Returns the median of a time over the repeats at sweep step `step`, in milliseconds.
fn median_at(
    reports: &[&ProbeReport],
    step: u32,
    time: fn(&ProbeResult) -> Option<Nanos>,
) -> Option<f64> {
    let v: Vec<f64> = reports
        .iter()
        .flat_map(|r| &r.results)
        .filter(|r| r.iteration % PHASES == step)
        .filter_map(|r| time(r).map(Nanos::as_millis_f64))
        .collect();
    median(&v)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hz: f64 = value("--hz").map_or(Ok(60.0), |v| v.parse())?;
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(hz)))
        .app_marks(true)
        .start()?;
    let argv = testclient_argv(&[
        "--app-id",
        "example.hover",
        "--scene",
        "hover",
        "--mode",
        "on-demand",
        "--render-delay",
        "7ms",
    ]);
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    let me = WindowRef::app_id("example.hover");
    if let Err(e) = session.blocking_wait_window(me.clone(), Nanos::from_secs(10)) {
        return Err(format!("no window ({e}): {}", app.stderr()).into());
    }
    session.blocking_wait_settled(me.clone(), Nanos::from_millis(200), Nanos::from_secs(5))?;

    let pixels = session.blocking_probe(probe(
        &me,
        Detector::PixelsChanged {
            roi: Roi::window_rect(me.clone(), TARGET),
            threshold: 0.01,
        },
    ))?;
    let marks = session.blocking_probe(probe(&me, Detector::AppMark { name: None }))?;
    let reports = [("pixels changed", &pixels), ("app mark", &marks)];

    let period = 1000.0 / hz;
    println!(
        "hover latency at {hz} Hz ({period:.2} ms period), 7 ms render, {REPEATS} repeats, ms\n"
    );
    println!("{:<18}| {:^20} | {:^20}", "", reports[0].0, reports[1].0);
    println!(
        "{:<18}| {:>6} {:>6} {:>6} | {:>6} {:>6} {:>6}",
        "stage", "p50", "p95", "max", "p50", "p95", "max"
    );
    for (name, stage) in STAGES {
        println!(
            "{name:<18}| {} | {}",
            cell(&pixels, stage),
            cell(&marks, stage)
        );
    }
    for (name, r) in reports {
        let readbacks: u32 = r.results.iter().map(|x| x.readbacks).sum();
        let cost: Nanos = r
            .results
            .iter()
            .map(|x| x.readback_cost)
            .fold(Nanos::ZERO, |a, b| a + b);
        println!(
            "{name}: {} timeouts, {readbacks} readbacks, {:.2} ms readback cost",
            r.timeouts(),
            cost.as_millis_f64()
        );
    }

    println!("\nmedian total by injection phase (a commit after the latch waits one refresh)");
    println!(
        "{:>12} | {:>14} | {:>8}",
        "input phase", reports[0].0, reports[1].0
    );
    let show = |v: Option<f64>| v.map_or("-".into(), |t| format!("{t:.2} ms"));
    for step in 0..PHASES {
        let total = |r: &ProbeResult| r.stages.total;
        println!(
            "{:>12} | {:>14} | {:>8}",
            show(median_at(&[&pixels, &marks], step, |r| r.phase_in_frame)),
            show(median_at(&[&pixels], step, total)),
            show(median_at(&[&marks], step, total))
        );
    }
    session.blocking_shutdown()?;
    Ok(())
}
