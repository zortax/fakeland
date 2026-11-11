//! A complex Rust bench. Part one scrolls at several velocities across 60, 120 and 144 Hz and two
//! output scales, one report variant per combination, and measures smoothness and pacing from a
//! capture of every presented frame. Part two runs a reactor-driven ping-pong latency loop on the
//! compositor thread at 60 Hz and at 144 Hz and compares the two reports.
//!
//! Run with `just example scroll_matrix`, or `cargo run -p fakeland-examples --example
//! scroll_matrix` after `cargo build -p fakeland-testclient`. `--quick` runs one velocity, one
//! scale and two rates. Arguments after `--` replace the reference client in part one, for
//! example `-- foot -e less /usr/share/dict/words`.
//!
//! The three reports go to `target/examples/scroll_matrix*.json`. `scroll_matrix_plot.py` plots
//! them into `target/examples/scroll_matrix.png`; it needs matplotlib.

use fakeland::scenario::{Position, Scenario, ScrollGlide, ScrollSource};
use fakeland::{
    CaptureSpec, Fakeland, LaunchSpec, ModeConfig, OutputChange, OutputConfig, Session, WindowRef,
};
use fakeland_bench::analyzers::{FramePacing, ScrollSmoothness};
use fakeland_bench::reactors::{PingPong, PingPongLatency};
use fakeland_bench::{Bench, BenchError, Thresholds, Variant, compare};
use fakeland_examples::{out_dir, testclient};
use fakeland_types::{Dur, Hz, Nanos, Rect};
use serde::Deserialize;
use serde_json::json;

/// The driver values of one matrix cell.
#[derive(Deserialize)]
struct Cell {
    velocity: f64,
}

fn launch(session: &Session, argv: Vec<String>, matcher: WindowRef) -> Result<(), BenchError> {
    let spec =
        LaunchSpec::from_argv(argv).ok_or_else(|| BenchError::Driver("empty command".into()))?;
    let app = session.blocking_launch(spec)?;
    session
        .blocking_wait_window(matcher, Nanos::from_secs(15))
        .map_err(|e| BenchError::Driver(format!("no window ({e}): {}", app.stderr())))?;
    Ok(())
}

/// Scroll glides over refresh rates, scales and velocities.
fn scroll_matrix(
    session: &Session,
    quick: bool,
    app: Option<Vec<String>>,
) -> Result<(), BenchError> {
    let target = match app {
        Some(argv) => {
            launch(session, argv, WindowRef::Focused)?;
            WindowRef::Focused
        }
        None => {
            let argv = [
                testclient().as_str(),
                "--app-id",
                "matrix.scroll",
                "--scene",
                "scroll",
            ]
            .map(String::from)
            .to_vec();
            launch(session, argv, WindowRef::app_id("matrix.scroll"))?;
            WindowRef::app_id("matrix.scroll")
        }
    };
    let rates: &[f64] = if quick {
        &[60.0, 144.0]
    } else {
        &[60.0, 120.0, 144.0]
    };
    let scales: &[f64] = if quick { &[1.0] } else { &[1.0, 1.5] };
    let velocities: &[f64] = if quick { &[1200.0] } else { &[600.0, 2400.0] };
    let mut bench = Bench::new("scroll-matrix")
        .target(target)
        .warmup(1)
        .iterations(2)
        .settle(Dur::from_millis(300))
        .scenario_fn(|cx| {
            let cell: Cell = cx.variant_values().unwrap_or(Cell { velocity: 1200.0 });
            let sign = if cx.index % 2 == 0 { 1.0 } else { -1.0 };
            Scenario::builder("glide")
                .scroll_glide(
                    ScrollGlide::constant(sign * cell.velocity, Dur::from_millis(800))
                        .at(Position::center(cx.window()))
                        .source(ScrollSource::wheel(true)),
                )
                .sleep(Dur::from_millis(250))
                .build()
        })
        .capture_frames(
            CaptureSpec::every_presented(WindowRef::Focused).with_roi(Rect::new(200, 40, 400, 400)),
        )
        .analyze(ScrollSmoothness::default())
        .analyze(FramePacing::default());
    for &hz in rates {
        for &scale in scales {
            for &velocity in velocities {
                bench = bench.variant(
                    Variant::new(format!("{hz}Hz x{scale} {velocity}px/s"))
                        .output(OutputChange::Mode(ModeConfig::new(0, 0, Hz(hz))))
                        .output(OutputChange::Scale { scale })
                        .values(json!({ "velocity": velocity })),
                );
            }
        }
    }
    let report = bench.blocking_run(session)?.into_report();
    println!("{}", report.to_text());
    save(&report, "scroll_matrix.json")
}

/// Writes a report as JSON into the example output directory.
fn save(report: &fakeland_bench::BenchReport, name: &str) -> Result<(), BenchError> {
    let path = out_dir().join(name);
    std::fs::write(&path, report.to_json_pretty())
        .map_err(|e| BenchError::Driver(format!("{}: {e}", path.display())))?;
    eprintln!("report: {}", path.display());
    Ok(())
}

/// The ping-pong loop on an output at one refresh rate.
fn ping_pong(session: &Session, hz: f64) -> Result<fakeland_bench::BenchReport, BenchError> {
    let me = WindowRef::app_id("matrix.hover");
    let bench = Bench::new(format!("ping-pong {hz}Hz"))
        .target(me)
        .output(OutputChange::Mode(ModeConfig::new(0, 0, Hz(hz))))
        .warmup(1)
        .iterations(5)
        .reactor(Dur::from_secs(10), |cx| {
            let w = cx.window();
            PingPong::new(
                Position::window(w.clone(), 200.0, 175.0),
                Position::window(w, 20.0, 20.0),
                20,
            )
        })
        .analyze(PingPongLatency);
    Ok(bench.blocking_run(session)?.into_report())
}

fn main() -> Result<(), BenchError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quick = args.iter().any(|a| a == "--quick");
    let app = args
        .iter()
        .position(|a| a == "--")
        .map(|i| args[i + 1..].to_vec())
        .filter(|a| !a.is_empty());
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()
        .map_err(|e| BenchError::Driver(e.to_string()))?;
    scroll_matrix(&session, quick, app)?;

    let argv = [
        testclient().as_str(),
        "--app-id",
        "matrix.hover",
        "--scene",
        "hover",
        "--mode",
        "on-demand",
        "--render-delay",
        "4ms",
    ]
    .map(String::from)
    .to_vec();
    launch(&session, argv, WindowRef::app_id("matrix.hover"))?;
    let slow = ping_pong(&session, 60.0)?;
    let fast = ping_pong(&session, 144.0)?;
    println!("{}", slow.to_text());
    println!("{}", fast.to_text());
    save(&slow, "scroll_matrix-pingpong-60.json")?;
    save(&fast, "scroll_matrix-pingpong-144.json")?;
    println!(
        "{}",
        compare(&slow, &fast, &Thresholds::default()).to_text()
    );
    session.blocking_shutdown()?;
    Ok(())
}
