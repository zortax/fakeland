//! Video recording and resource sampling. A 60 Hz session samples the CPU, memory and threads of
//! each app every 100 ms, and the GPU when the host reports it. The example records three seconds
//! of the output, one frame per vblank, and prints the recording report and the latest resource
//! samples.
//!
//! Run with `just example record_video`, or `cargo run -p fakeland-examples --example
//! record_video` after `cargo build -p fakeland-testclient`. It runs the reference client with an
//! animated bar; arguments after `--` run another app, for example `-- vkcube --wsi wayland`. The
//! video goes to `target/examples/record_video.mp4` when `ffmpeg` is on the `PATH`, and to
//! `target/examples/record_video.y4m` otherwise.

use std::time::Duration;

use fakeland::{
    Fakeland, LaunchSpec, OutputConfig, RecordSpec, SamplingConfig, VideoFormat, WindowRef,
};
use fakeland_examples::{app_command, out_dir, testclient_argv};
use fakeland_types::{Hz, Nanos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .sampling(SamplingConfig::every(Nanos::from_millis(100)))
        .start()?;
    let argv = app_command().unwrap_or_else(|| {
        testclient_argv(&["--scene", "animate", "--speed", "400", "--show-frame-id"])
    });
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .map_err(|e| format!("no window ({e}): {}", app.stderr()))?;
    session.blocking_wait_frames(WindowRef::Any, 30, Nanos::from_secs(10))?;

    let (name, format) = if on_path("ffmpeg") {
        ("record_video.mp4", VideoFormat::Mp4)
    } else {
        ("record_video.y4m", VideoFormat::Y4m)
    };
    let path = out_dir().join(name);
    let _ = std::fs::remove_file(&path);
    let recording = session.blocking_record(RecordSpec::output(&path).format(format))?;
    println!("recording {} for 3 s", format.as_str());
    std::thread::sleep(Duration::from_secs(3));
    let report = recording.blocking_stop()?;
    println!("{}", report.to_text());
    let slots = report.stopped.since(report.first_slot).as_secs_f64() * report.rate.hz().0;
    println!(
        "{:.0} vblank slots from the first frame to the stop hold {} frames",
        slots.floor() + 1.0,
        report.frames
    );

    let resources = session.blocking_resources()?;
    println!(
        "resources, sampled every {:.0} ms:",
        resources.interval.as_millis_f64()
    );
    print!("{}", resources.to_text());
    session.blocking_shutdown()?;
    if report.ok() {
        Ok(())
    } else {
        Err(report.error.unwrap_or_default().into())
    }
}

/// Returns true when an executable of this name is in a `PATH` directory.
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}
