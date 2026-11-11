//! `fakeland run`: run an app headless for a while, write its trace and print a frame summary.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use fakeland::{LaunchSpec, RecordSpec, Session};
use fakeland_control::Controller;
use fakeland_control::export::{export_trace, trace_info};
use fakeland_control::methods::{LaunchParams, TraceFormat, TraceOutput};
use fakeland_control::ops::{describe_exit, shot_info, tail_lines};
use fakeland_control::text::{summary_text, validity_text};
use fakeland_scenario::{Action, Anchor, CaptureTarget, Scenario, Step};
use fakeland_trace::Trace;
use fakeland_types::{Dur, Hz};

use super::resources::resources_text;
use crate::session_args::SessionArgs;
use crate::util::{absolute, parse_secs, print_json, signal};

/// A screenshot at a time after launch.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotAt {
    /// The time after launch.
    pub at: Dur,
    /// The PNG file.
    pub path: PathBuf,
}

fn parse_shot_at(s: &str) -> Result<ShotAt, String> {
    let (at, path) = s
        .split_once(':')
        .ok_or_else(|| format!("{s:?} is not TIME:FILE"))?;
    if path.is_empty() {
        return Err(format!("{s:?} names no file"));
    }
    Ok(ShotAt {
        at: parse_secs(at)?,
        path: PathBuf::from(path),
    })
}

/// Flags of `fakeland run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    session: SessionArgs,
    /// Write the trace: `.jsonl` gives JSON lines, `.json` one document.
    #[arg(long, value_name = "FILE")]
    trace: Option<PathBuf>,
    /// Write a Chrome trace for ui.perfetto.dev.
    #[arg(long, value_name = "FILE")]
    perfetto: Option<PathBuf>,
    /// Write one CSV row per frame.
    #[arg(long, value_name = "FILE")]
    csv: Option<PathBuf>,
    /// Write the JSON summary.
    #[arg(long, value_name = "FILE")]
    summary: Option<PathBuf>,
    /// Stop after this time, such as 10s. Without it the run lasts until the app exits or a
    /// signal arrives.
    #[arg(long, value_name = "DUR", value_parser = parse_secs)]
    duration: Option<Dur>,
    /// Take a screenshot of the primary output at a time after launch, such as `2s:out.png`.
    /// Repeat for more.
    #[arg(long = "screenshot-at", value_name = "TIME:FILE", value_parser = parse_shot_at)]
    screenshots: Vec<ShotAt>,
    /// Downscale screenshots so the longer side is at most this many pixels.
    #[arg(long, value_name = "PX")]
    max_dim: Option<u32>,
    /// Record the primary output from launch to the end of the run: `.mp4`, `.mkv`, `.mov`,
    /// `.y4m`, or a directory of PNG frames.
    #[arg(long, value_name = "FILE")]
    record: Option<PathBuf>,
    /// The frame rate of the recording. The default is one frame per refresh.
    #[arg(long, value_name = "HZ", requires = "record")]
    record_fps: Option<f64>,
    /// Downscale the recording so the longer side is at most this many pixels.
    #[arg(long, value_name = "PX", requires = "record")]
    record_max_dim: Option<u32>,
    /// Print the summary as JSON.
    #[arg(long)]
    json: bool,
    /// The command to run, after `--`.
    #[arg(required = true, last = true, value_name = "CMD")]
    command: Vec<String>,
}

fn shot_scenario(shots: &[ShotAt], max_dim: Option<u32>) -> Scenario {
    let mut sorted = shots.to_vec();
    sorted.sort_by_key(|s| s.at.0);
    Scenario {
        name: "screenshots".into(),
        steps: sorted
            .iter()
            .map(|s| {
                Step::new(Action::Capture {
                    target: CaptureTarget::default(),
                    label: Some(s.path.to_string_lossy().into_owned()),
                    max_dim,
                })
                .anchor(Anchor::At { at: s.at })
            })
            .collect(),
        triggers: Vec::new(),
        timeout: None,
    }
}

fn outputs(args: &RunArgs) -> Result<Vec<TraceOutput>> {
    let mut out = Vec::new();
    let mut add = |path: &Option<PathBuf>, format: Option<TraceFormat>| -> Result<()> {
        if let Some(p) = path {
            out.push(TraceOutput {
                path: absolute(p)?,
                format,
            });
        }
        Ok(())
    };
    add(&args.trace, None)?;
    add(&args.perfetto, Some(TraceFormat::Perfetto))?;
    add(&args.csv, Some(TraceFormat::Csv))?;
    add(&args.summary, Some(TraceFormat::Summary))?;
    Ok(out)
}

/// Runs `fakeland run` and returns the process exit code.
pub async fn run(args: RunArgs) -> Result<i32> {
    let outputs = outputs(&args)?;
    let spec = LaunchSpec::from_argv(args.command.clone()).context("no command")?;
    let session_args = args.session.clone();
    let session = tokio::task::spawn_blocking(move || session_args.start()).await??;
    let result = drive(&session, &args, spec, &outputs).await;
    let shutdown = session.clone();
    tokio::task::spawn_blocking(move || shutdown.blocking_shutdown()).await??;
    result
}

async fn drive(
    session: &Session,
    args: &RunArgs,
    spec: LaunchSpec,
    outputs: &[TraceOutput],
) -> Result<i32> {
    let controller = Controller::new(session.clone());
    let capture = session.begin_capture().await?;
    let info = controller
        .launch(LaunchParams {
            spec,
            wait_window: None,
        })
        .await?;
    eprintln!(
        "fakeland: {} runs as pid {} on WAYLAND_DISPLAY={}",
        info.command,
        info.pid.0,
        session.socket_name()
    );
    let app = controller.app(Some(info.pid))?;
    let recording = match &args.record {
        Some(path) => {
            let spec = RecordSpec {
                fps: args.record_fps.map(Hz),
                max_dim: args.record_max_dim,
                ..RecordSpec::output(absolute(path)?)
            };
            Some(session.record(spec).await?)
        }
        None => None,
    };
    let shots = match args.screenshots.is_empty() {
        true => None,
        false => Some(
            session
                .run(shot_scenario(&args.screenshots, args.max_dim))
                .await?,
        ),
    };
    let duration = args.duration.map(|d| d.0.to_duration());
    let reason = tokio::select! {
        _ = async {
            match duration {
                Some(d) => tokio::time::sleep(d).await,
                None => std::future::pending().await,
            }
        } => "duration",
        _ = app.wait() => "app exit",
        _ = signal() => "signal",
    };
    eprintln!("fakeland: stopping after {reason}");
    if let Some(mut handle) = shots {
        let result = match handle.try_result() {
            Some(r) => r,
            None => {
                handle.cancel().await?;
                handle.await?
            }
        };
        for c in &result.captures {
            let path = absolute(&PathBuf::from(c.label.clone().unwrap_or_default()))?;
            let shot = shot_info(&c.screenshot, Some(&path), false)?;
            eprintln!(
                "fakeland: screenshot {}x{} -> {}",
                shot.width,
                shot.height,
                path.display()
            );
        }
    }
    if let Some(recording) = recording {
        let r = recording.stop().await?;
        eprintln!("fakeland: video {}", r.to_text());
        if let Some(e) = &r.error {
            bail!("the recording failed: {e}");
        }
    }
    let trace = session.end_capture(capture).await?;
    let validity = session.validity().await?;
    report(&trace, outputs, &validity, args.json)?;
    if session.config().sampling.is_some() && !args.json {
        print!("{}", resources_text(&trace));
    }
    let exit = app.try_wait();
    if let Some(e) = exit {
        eprintln!(
            "fakeland: {} ended with {}",
            app.command(),
            describe_exit(e.code, e.signal)
        );
        if !e.success() && reason == "app exit" {
            let tail = tail_lines(&app.stderr(), 20);
            if !tail.is_empty() {
                eprintln!("fakeland: stderr of the app:\n{tail}");
            }
            return Ok(e.code.unwrap_or(1));
        }
    }
    Ok(0)
}

fn report(
    trace: &Trace,
    outputs: &[TraceOutput],
    validity: &fakeland::RunValidity,
    json: bool,
) -> Result<()> {
    let mut info = None;
    for out in outputs {
        let written = export_trace(trace, out)?;
        eprintln!(
            "fakeland: {} trace -> {}",
            out.format().as_str(),
            out.path.display()
        );
        info = Some(written);
    }
    let info = info.unwrap_or_else(|| trace_info(trace));
    if json {
        print_json(&serde_json::json!({ "trace": info, "validity": validity }))?;
    } else {
        print!("{}", summary_text(&info.summary));
        println!("{}", validity_text(validity));
    }
    if !validity.is_valid() {
        eprintln!("fakeland: compositor lateness passed the threshold; timing is unreliable");
    }
    if trace.records.is_empty() {
        bail!("the trace is empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_screenshot_times() {
        let s = parse_shot_at("2s:out.png").unwrap();
        assert_eq!(s.at, Dur::from_secs(2));
        assert_eq!(s.path, PathBuf::from("out.png"));
        let s = parse_shot_at("1.5:/tmp/a:b.png").unwrap();
        assert_eq!(s.at, Dur::from_millis(1500));
        assert_eq!(s.path, PathBuf::from("/tmp/a:b.png"));
        assert!(parse_shot_at("2s").is_err());
        assert!(parse_shot_at("2s:").is_err());
    }

    #[test]
    fn orders_screenshot_steps() {
        let shots = [
            ShotAt {
                at: Dur::from_secs(3),
                path: "b.png".into(),
            },
            ShotAt {
                at: Dur::from_secs(1),
                path: "a.png".into(),
            },
        ];
        let s = shot_scenario(&shots, Some(800));
        s.validate().unwrap();
        assert_eq!(
            s.steps[0].anchor,
            Anchor::At {
                at: Dur::from_secs(1)
            }
        );
        assert_eq!(s.steps[1].label, None);
    }
}
