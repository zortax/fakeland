//! Latency attribution of a zgui app through its fakeland app marks.
//!
//! The example starts a session with app marks, launches a zgui example built with the
//! `fakeland-marks` feature, and probes it. Hover and click probes run with the app-mark detector
//! and with the pixel detector. A scroll glide compares the scroll that the marks attribute to
//! each frame with the scroll that the pixels show. The output splits each latency into the
//! stages from input send to presentation, including the app's own frame phases, and checks that
//! the commit the app marks as handling an input is the commit whose pixels change. The session
//! trace goes to a Perfetto JSON file.
//!
//! Build the zgui example first, in the zgui checkout:
//! `cargo build --release -p zgui-examples --example counter --features zgui/fakeland-marks`.
//! Then run
//! `cargo run --release -p fakeland-bench --example zgui_marks -- -- <zgui>/target/release/examples/counter`.
//! The examples `counter` (hover and click) and `styled` (scroll) have built-in targets; other
//! binaries take `--hover`, `--click` and `--scroll` flags.

mod args;
mod breakdown;
mod checks;
mod perfetto;
mod probes;
mod scroll;

use std::process::ExitCode;

use fakeland::{CaptureOptions, CaptureTarget, Fakeland, LaunchSpec, OutputConfig, Session};
use fakeland_trace::frames::FrameTimeline;
use fakeland_types::{Hz, Nanos};

use crate::args::Args;
use crate::probes::{Kind, Plan};

type Error = Box<dyn std::error::Error>;

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match Args::parse(&raw) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("\nFAILED: a check did not hold");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Runs everything and returns whether every check held.
fn run(args: &Args) -> Result<bool, Error> {
    std::fs::create_dir_all(&args.out)?;
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(args.size.0, args.size.1, Hz(args.hz)))
        .app_marks(true)
        .start()?;
    let result = measure(&session, args);
    session.blocking_shutdown()?;
    result
}

fn measure(session: &Session, args: &Args) -> Result<bool, Error> {
    let capture = session.blocking_begin_capture()?;
    let spec = LaunchSpec::from_argv(args.command.clone()).ok_or("empty command")?;
    let app = session.blocking_launch(spec)?;
    let window = app
        .blocking_wait_window(Nanos::from_secs(20))
        .map_err(|e| format!("no window ({e}): {}", app.stderr()))?;
    session.blocking_wait_settled(
        window.id.into(),
        Nanos::from_millis(300),
        Nanos::from_secs(10),
    )?;
    let shot = session.blocking_screenshot(
        CaptureTarget::Window {
            window: window.id.into(),
        },
        CaptureOptions::default(),
    )?;
    std::fs::write(args.out.join("start.png"), shot.image.encode_png()?)?;
    println!(
        "zgui app {} pid {} window {} {}x{} on {}x{}@{} Hz",
        args.command[0],
        app.pid().0,
        window.id.0,
        window.geometry.w,
        window.geometry.h,
        args.size.0,
        args.size.1,
        args.hz
    );
    fakeland::block_on(session.reset_validity())?;

    let plan = Plan {
        window: window.id,
        repeats: args.repeats,
    };
    let mut runs = Vec::new();
    if let Some(hover) = args.hover {
        runs.push(plan.hover(session, hover, Kind::AppMark)?);
        runs.push(plan.hover(session, hover, Kind::Pixels)?);
    }
    if let Some(click) = args.click {
        runs.push(plan.click(session, click, Kind::AppMark)?);
        runs.push(plan.click(session, click, Kind::Pixels)?);
    }
    let mut glides = Vec::new();
    if let Some(s) = args.scroll {
        for sign in [1.0, -1.0] {
            glides.push(scroll::run(session, &window, s, sign)?);
        }
    }
    let validity = session.blocking_validity()?;
    let trace = session.blocking_end_capture(capture)?;
    let timeline = FrameTimeline::build(&trace);

    let mut ok = true;
    for run in &runs {
        let list = breakdown::repeats(run, &timeline);
        let (agree, total) = breakdown::print(run, &list, args.verbose);
        let timed_out = run.report.timeouts();
        ok &= timed_out == 0 && agree == total;
    }
    for glide in &glides {
        scroll::print(glide, args.verbose);
        ok &= glide.moving > 0 && glide.agree == glide.moving && glide.marks_only == 0;
    }
    let frames = checks::frames(&timeline);
    ok &= frames.ok();
    checks::inputs(&timeline);
    let startup = checks::startup(&trace, &timeline);
    ok &= ["connected", "configured", "first_frame"]
        .iter()
        .all(|name| startup.iter().any(|(n, _)| n == name));
    let jsonl = args.out.join("zgui-marks.trace.jsonl");
    trace.write_jsonl(std::io::BufWriter::new(std::fs::File::create(&jsonl)?))?;
    println!(
        "\ntrace: {} records in {}",
        trace.records.len(),
        jsonl.display()
    );
    let path = args.out.join("zgui-marks.perfetto.json");
    let contents = perfetto::write(&trace, &timeline, &path)?;
    ok &= contents.ok();
    println!(
        "\nvalidity: {} wakes, {} late, max lateness {:.3} ms",
        validity.wakes,
        validity.late_wakes,
        validity.max_lateness.as_millis_f64()
    );
    Ok(ok)
}
