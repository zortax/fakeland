//! The fake display. Part one runs the same animating client for 1.5 s under five display clock
//! models: fixed 60 Hz, fixed 144 Hz, VRR 48 to 144 Hz with a 90 Hz client, fixed 60 Hz with
//! latch jitter and a dropped vblank every 10, and unthrottled. Under the jitter, the client
//! renders for 14 ms, so a late latch misses some of its frames. It prints one row of frame
//! pacing per model: present rate, present interval percentiles, missed and dropped vblanks, and
//! the refresh and flags of the presentation feedback. Part two switches the refresh rate and the
//! scale of a running output. Part three steps a manual output: nothing presents until a step,
//! each step presents one frame, and a frame capture keeps the image of each presented frame.
//!
//! Run with `just example display_timing`. It runs the reference client; arguments after `--`
//! run another app in every part, for example `-- vkcube --wsi wayland`. The images of the manual
//! steps go to `target/examples/display_timing-step-N.png`.

use std::time::Duration;

use fakeland::{
    CaptureSpec, Event, Fakeland, Image, LaunchSpec, ModeConfig, OutputChange, OutputConfig,
    Perturbation, Session, Trace, WindowInfo, WindowRef,
};
use fakeland_bench::analyzers::FramePacing;
use fakeland_bench::{AnalysisInput, Analyzer};
use fakeland_examples::{app_command, init_logging, out_dir, testclient_argv};
use fakeland_timing::{PresentFlags, TimingConfig};
use fakeland_types::{Hz, Nanos, SurfaceId};
use fakeland_vision::diff;

type Error = Box<dyn std::error::Error>;

const SIZE: (i32, i32) = (1280, 720);

/// One display clock model and the client arguments that suit it.
struct Mode {
    name: &'static str,
    refresh: Hz,
    timing: TimingConfig,
    client: &'static [&'static str],
}

/// The models of part one.
fn modes() -> Vec<Mode> {
    let animate: &[&str] = &["--scene", "animate"];
    vec![
        Mode {
            name: "fixed 60 Hz",
            refresh: Hz(60.0),
            timing: TimingConfig::fixed(Hz(60.0)),
            client: animate,
        },
        Mode {
            name: "fixed 144 Hz",
            refresh: Hz(144.0),
            timing: TimingConfig::fixed(Hz(144.0)),
            client: animate,
        },
        Mode {
            name: "VRR 48-144, 90 Hz app",
            refresh: Hz(144.0),
            timing: TimingConfig::vrr(Hz(48.0), Hz(144.0)),
            client: &["--scene", "animate", "--mode", "fixed-rate", "--rate", "90"],
        },
        Mode {
            name: "60 Hz, jitter, drop/10",
            refresh: Hz(60.0),
            timing: TimingConfig::fixed(Hz(60.0)).with_perturbation(Perturbation {
                latch_jitter: Nanos::from_millis(2),
                drop_every: Some(10),
                seed: 7,
                ..Perturbation::default()
            }),
            client: &["--scene", "animate", "--render-delay", "14ms"],
        },
        Mode {
            name: "unthrottled",
            refresh: Hz(60.0),
            timing: TimingConfig::unthrottled(),
            client: &["--scene", "animate", "--mode", "unthrottled"],
        },
    ]
}

/// Starts a session with one 1280x720 output.
fn start(refresh: Hz, timing: TimingConfig) -> Result<Session, Error> {
    let output = OutputConfig::new("DP-1")
        .mode(SIZE.0, SIZE.1, refresh)
        .timing(timing);
    Ok(Fakeland::builder().output(output).start()?)
}

/// Launches the app after `--`, or the reference client with `client` arguments, and waits for
/// its window.
fn launch(session: &Session, client: &[&str]) -> Result<WindowInfo, Error> {
    let argv = app_command().unwrap_or_else(|| testclient_argv(client));
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    app.blocking_wait_window(Nanos::from_secs(10))
        .map_err(|e| format!("no window ({e}): {}", app.stderr()).into())
}

/// Records the session for a while.
fn capture(session: &Session, secs: f64) -> Result<Trace, Error> {
    let id = session.blocking_begin_capture()?;
    std::thread::sleep(Duration::from_secs_f64(secs));
    Ok(session.blocking_end_capture(id)?)
}

/// Prints the pacing row of one surface in a trace.
fn print_row(name: &str, trace: Trace, surface: SurfaceId) {
    let pacing = FramePacing {
        surface: Some(surface),
        ..FramePacing::default()
    };
    let dropped = trace
        .records
        .iter()
        .filter(|r| matches!(r.event, Event::Vblank { dropped: true, .. }))
        .count();
    let (flags, refresh) = trace
        .records
        .iter()
        .find_map(|r| match r.event {
            Event::Presented {
                surface: s,
                flags,
                refresh,
                ..
            } if s == surface => Some((flags, refresh)),
            _ => None,
        })
        .unwrap_or_default();
    let report = pacing.analyze(&AnalysisInput::new(trace));
    let (p50, p99) = report
        .interval_ms
        .map_or((f64::NAN, f64::NAN), |s| (s.p50, s.p99));
    let refresh = refresh.map_or("-".to_owned(), |r| {
        format!("{:.2} ms", r.as_nanos() as f64 / 1e6)
    });
    let flags = format!("{:?}", PresentFlags(flags));
    let flags = flags
        .trim_start_matches("PresentFlags(")
        .trim_end_matches(')');
    println!(
        "{name:<24} {:>7.1} {:>8.2} {:>8.2} {:>6} {:>7} {:>9}  {flags}",
        report.fps.unwrap_or(f64::NAN),
        p50,
        p99,
        report.missed_vblanks,
        dropped,
        refresh,
    );
}

/// Part one: one session per display clock model.
fn compare_modes() -> Result<(), Error> {
    println!(
        "{:<24} {:>7} {:>8} {:>8} {:>6} {:>7} {:>9}  flags",
        "display", "fps", "p50 ms", "p99 ms", "missed", "dropped", "refresh"
    );
    for mode in modes() {
        let session = start(mode.refresh, mode.timing)?;
        let window = launch(&session, mode.client)?;
        std::thread::sleep(Duration::from_millis(300));
        let trace = capture(&session, 1.5)?;
        let validity = session.blocking_validity()?;
        session.blocking_shutdown()?;
        print_row(mode.name, trace, window.surface);
        if !validity.is_valid() {
            println!(
                "  invalid run: {} late wakes, max {:.2} ms",
                validity.late_wakes,
                validity.max_lateness.as_nanos() as f64 / 1e6
            );
        }
    }
    Ok(())
}

/// Part two: switches a running output from 60 Hz at scale 1 to 120 Hz at scale 2.
fn switch_mode() -> Result<(), Error> {
    println!("\nruntime switch of one output:");
    let session = start(Hz(60.0), TimingConfig::default())?;
    let window = launch(&session, &["--scene", "animate"])?;
    std::thread::sleep(Duration::from_millis(300));
    print_row("before: 60 Hz, x1", capture(&session, 0.8)?, window.surface);
    session.blocking_set_output(
        None,
        OutputChange::Mode(ModeConfig::new(SIZE.0, SIZE.1, Hz(120.0))),
    )?;
    session.blocking_set_output(None, OutputChange::Scale { scale: 2.0 })?;
    std::thread::sleep(Duration::from_millis(300));
    print_row("after: 120 Hz, x2", capture(&session, 0.8)?, window.surface);
    for out in session.blocking_outputs()? {
        println!(
            "  {} now {}x{}@{} Hz, scale {}, logical {}x{}",
            out.name,
            out.mode.width,
            out.mode.height,
            out.mode.refresh.0,
            out.scale,
            out.logical_size.w,
            out.logical_size.h
        );
    }
    session.blocking_shutdown()?;
    Ok(())
}

/// Counts the presented frames of one surface in a trace.
fn presented(trace: &Trace, surface: SurfaceId) -> usize {
    trace
        .records
        .iter()
        .filter(|r| matches!(r.event, Event::Presented { surface: s, .. } if s == surface))
        .count()
}

/// Part three: a manual output presents one frame per step.
fn manual_steps() -> Result<(), Error> {
    println!("\nmanual output, one frame per step:");
    let session = start(Hz(60.0), TimingConfig::manual(Hz(60.0)))?;
    let argv = app_command().unwrap_or_else(|| {
        testclient_argv(&["--scene", "animate", "--show-frame-id", "--speed", "200"])
    });
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    // The window maps with its first presented frame, so step until it shows.
    let window = loop {
        session.blocking_step(None, 1)?;
        match session.blocking_wait_window(WindowRef::Any, Nanos::from_millis(100)) {
            Ok(w) => break w,
            Err(_) if app.try_wait().is_none() => continue,
            Err(e) => return Err(format!("no window ({e}): {}", app.stderr()).into()),
        }
    };
    // The client renders its next frame after each step; give it time to commit.
    let settle = || std::thread::sleep(Duration::from_millis(50));
    settle();
    let frames = session
        .blocking_begin_frame_capture(CaptureSpec::every_presented(WindowRef::id(window.id)))?;
    let idle = capture(&session, 0.3)?;
    println!(
        "  300 ms without a step: {} frames presented",
        presented(&idle, window.surface)
    );
    let mut per_step = Vec::new();
    for _ in 0..4 {
        let id = session.blocking_begin_capture()?;
        session.blocking_step(None, 1)?;
        settle();
        per_step.push(presented(
            &session.blocking_end_capture(id)?,
            window.surface,
        ));
    }
    let frames = session.blocking_end_frame_capture(frames)?.frames;
    let mut last: Option<&Image> = None;
    for (n, (frame, count)) in frames.iter().zip(&per_step).enumerate() {
        let path = out_dir().join(format!("display_timing-step-{}.png", n + 1));
        std::fs::write(&path, frame.image.encode_png()?)?;
        let changed = match last {
            Some(prev) => diff(prev, &frame.image, None, 0)?
                .changed_pixels
                .to_string(),
            None => "-".to_owned(),
        };
        println!(
            "  step {}: {count} frame presented at vblank {}, {changed:>5} pixels changed, {}",
            n + 1,
            frame.seq.unwrap_or_default(),
            path.display()
        );
        last = Some(&frame.image);
    }
    session.blocking_shutdown()?;
    Ok(())
}

fn main() -> Result<(), Error> {
    init_logging();
    compare_modes()?;
    switch_mode()?;
    manual_steps()?;
    Ok(())
}
