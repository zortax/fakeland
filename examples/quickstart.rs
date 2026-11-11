//! The library quick start: a 144 Hz session at scale 1.25, an animating app, a screenshot, a
//! scenario and the frame pacing preset.
//!
//! Run with `just example quickstart`. It runs the reference client; arguments after `--` run
//! another app, for example `-- vkcube --wsi wayland`. The screenshot goes to
//! `target/examples/quickstart.png`.

use fakeland::{CaptureOptions, CaptureTarget, Fakeland, LaunchSpec, OutputConfig, WindowRef};
use fakeland_bench::presets::{self, PresetParams};
use fakeland_examples::{app_command, out_dir, testclient_argv};
use fakeland_scenario::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1920, 1080, Hz(143.912))
                .scale(1.25),
        )
        .app_marks(true)
        .start()?;
    let argv = app_command().unwrap_or_else(|| testclient_argv(&["--scene", "animate,hover"]));
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    let window = app.blocking_wait_window(Nanos::from_secs(10))?;
    println!("{:?} maps at {:?}", window.title, window.geometry);

    let shot = session.blocking_screenshot(
        CaptureTarget::Output { output: None },
        CaptureOptions {
            include_cursor: false,
            max_dim: Some(1280),
        },
    )?;
    let path = out_dir().join("quickstart.png");
    std::fs::write(&path, shot.image.encode_png()?)?;
    println!(
        "screenshot {}x{}, 1 px = {:.2} logical px: {}",
        shot.image.width(),
        shot.image.height(),
        shot.scale,
        path.display()
    );

    let scenario = Scenario::builder("hover")
        .at_next_vblank()
        .pointer_move(Position::center(WindowRef::Focused))
        .wait_frames(10)
        .build()?;
    let result = session.blocking_run(scenario)?.blocking_wait()?;
    println!("scenario ok: {}, {} steps", result.ok, result.steps.len());

    let bench = presets::by_name("pacing")
        .ok_or("no pacing preset")?
        .bench(&PresetParams::new())?;
    let report = bench.blocking_run(&session)?.into_report();
    println!("{}", report.to_text());
    session.blocking_shutdown()?;
    Ok(())
}
