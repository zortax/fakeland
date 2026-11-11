//! A regression check with the `Bench` API. One bench probes hover latency and then watches the
//! animation for frame pacing. It runs against a baseline app and against a candidate app with a
//! longer render time and periodic jank. `compare` checks the candidate report against the
//! baseline report and flags the regressed metrics.
//!
//! Run with `just example bench_compare`, or `cargo run -p fakeland-examples --example
//! bench_compare` after `cargo build -p fakeland-testclient`. The reports go to
//! `target/examples/bench_compare-baseline.json` and
//! `target/examples/bench_compare-candidate.json`. The exit code is 1 when the candidate
//! regresses.

use fakeland::scenario::{
    Action, Detector, PhaseSpec, PointerAction, Position, ProbeSpec, Scenario, WindowAction,
};
use fakeland::{Fakeland, LaunchSpec, OutputConfig, Session, WindowRef};
use fakeland_bench::analyzers::{FramePacing, Latency};
use fakeland_bench::compare::Verdict;
use fakeland_bench::{Bench, BenchReport, Thresholds, compare};
use fakeland_examples::{out_dir, testclient_argv};
use fakeland_types::{Dur, Hz, Nanos};

/// The app id of both apps, so one bench definition targets either.
const APP_ID: &str = "bench.compare";

/// Builds the bench: per iteration, ten hover probes at random vblank phases, then two seconds
/// of animation.
fn bench(name: &str) -> Bench {
    let window = WindowRef::app_id(APP_ID);
    Bench::new(name)
        .target(window.clone())
        .warmup(1)
        .iterations(3)
        .settle(Dur::from_millis(300))
        .scenario_fn(move |cx| {
            let w = cx.window();
            let probe = ProbeSpec::new(
                Action::Pointer(PointerAction::move_to(Position::window(
                    w.clone(),
                    200.0,
                    175.0,
                ))),
                Detector::AppMark { name: None },
            )
            .alternate(Action::Pointer(PointerAction::move_to(Position::window(
                w.clone(),
                20.0,
                20.0,
            ))))
            .repeat(10)
            .phase(PhaseSpec::Random {
                seed: u64::from(cx.index),
            })
            // The app animates and never goes quiet, so each repeat starts without a settle wait.
            .settle(Dur::ZERO)
            .label("hover");
            Scenario::builder("hover-then-watch")
                .window(WindowAction::Focus { window: w })
                .probe(probe)
                .sleep(Dur::from_secs(2))
                .build()
        })
        .analyze(Latency::default())
        .analyze(FramePacing::default())
}

/// Launches an app, runs the bench against it and stops the app.
fn run(
    session: &Session,
    name: &str,
    extra: &[&str],
) -> Result<BenchReport, Box<dyn std::error::Error>> {
    let mut args = vec!["--app-id", APP_ID, "--scene", "animate,hover"];
    args.extend_from_slice(extra);
    let spec = LaunchSpec::from_argv(testclient_argv(&args)).ok_or("empty command")?;
    let app = session.blocking_launch(spec)?;
    session.blocking_wait_window(WindowRef::app_id(APP_ID), Nanos::from_secs(10))?;
    println!("{name}: testclient {}", extra.join(" "));
    let report = bench(name).blocking_run(session)?.into_report();
    app.kill()?;
    app.blocking_wait();
    let path = out_dir().join(format!("bench_compare-{name}.json"));
    std::fs::write(&path, report.to_json_pretty())?;
    println!("{}\nreport: {}\n", report.to_text(), path.display());
    Ok(report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()?;
    let baseline = run(&session, "baseline", &["--render-delay", "2ms"])?;
    let candidate = run(
        &session,
        "candidate",
        &[
            "--render-delay",
            "9ms",
            "--jank-every",
            "6",
            "--jank",
            "25ms",
        ],
    )?;
    session.blocking_shutdown()?;

    let comparison = compare(&baseline, &candidate, &Thresholds::default());
    println!("{}", comparison.to_text());
    let regressed: Vec<String> = comparison
        .with_verdict(Verdict::Regressed)
        .map(|m| format!("{}.{}", m.section, m.name))
        .collect();
    println!("{} regressed: {}", regressed.len(), regressed.join(", "));
    if comparison.is_regression() {
        std::process::exit(1);
    }
    Ok(())
}
