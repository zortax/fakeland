//! `fakeland bench`: start a session, launch the app, run a preset or a scenario as a bench and
//! print its report.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use fakeland::LaunchSpec;
use fakeland_bench::Thresholds;
use fakeland_control::Controller;
use fakeland_control::methods::{
    BenchOutcome, BenchParams, LaunchParams, TraceFormat, TraceOutput, WaitSettledParams,
};
use fakeland_control::text::validity_text;
use fakeland_scenario::WindowRef;
use fakeland_types::Dur;

use crate::bench_args::BenchSelect;
use crate::session_args::SessionArgs;
use crate::util::{absolute, parse_secs, print_json};

/// The exit code of a bench that regressed against its baseline.
pub const REGRESSION_EXIT: i32 = 3;

/// Flags of `fakeland bench`.
#[derive(Debug, Args)]
pub struct BenchArgs {
    #[command(flatten)]
    select: BenchSelect,
    #[command(flatten)]
    session: SessionArgs,
    /// Write the report as JSON.
    #[arg(long, value_name = "FILE")]
    report: Option<PathBuf>,
    /// Write a Chrome trace of the whole bench for ui.perfetto.dev.
    #[arg(long, value_name = "FILE")]
    perfetto: Option<PathBuf>,
    /// Write the trace of the whole bench as JSONL.
    #[arg(long, value_name = "FILE")]
    trace: Option<PathBuf>,
    /// Write the trace of each iteration as JSONL into this directory.
    #[arg(long, value_name = "DIR")]
    traces_dir: Option<PathBuf>,
    /// Compare with a baseline report and exit with 3 on a regression.
    #[arg(long, value_name = "FILE")]
    baseline: Option<PathBuf>,
    /// A change must exceed this fraction of the baseline to count.
    #[arg(long, value_name = "FRACTION")]
    relative: Option<f64>,
    /// Wait this long for the first window of the app.
    #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "10s")]
    wait_window: Dur,
    /// Print the outcome as JSON.
    #[arg(long)]
    json: bool,
    /// The app to bench, after `--`. The startup preset launches it itself.
    #[arg(last = true, value_name = "CMD")]
    command: Vec<String>,
}

fn files(args: &BenchArgs, mut p: BenchParams) -> Result<BenchParams> {
    for (path, format) in [
        (&args.perfetto, TraceFormat::Perfetto),
        (&args.trace, TraceFormat::Jsonl),
    ] {
        if let Some(path) = path {
            p.traces.push(TraceOutput {
                path: absolute(path)?,
                format: Some(format),
            });
        }
    }
    p.traces_dir = args.traces_dir.as_deref().map(absolute).transpose()?;
    p.report = args.report.as_deref().map(absolute).transpose()?;
    p.baseline = args.baseline.as_deref().map(absolute).transpose()?;
    if let Some(r) = args.relative {
        p.thresholds = Some(Thresholds {
            relative: r,
            ..Thresholds::default()
        });
    }
    Ok(p)
}

/// Prints a bench outcome and returns the exit code.
pub fn print_outcome(outcome: &BenchOutcome, json: bool) -> Result<i32> {
    if json {
        print_json(outcome)?;
    } else {
        print!("{}", outcome.text);
        if let Some(t) = &outcome.comparison_text {
            println!("{t}");
        }
        for t in &outcome.traces {
            if let Some(p) = &t.path {
                eprintln!("fakeland: trace -> {}", p.display());
            }
        }
        for f in &outcome.files {
            eprintln!("fakeland: wrote {}", f.display());
        }
    }
    let regressed = outcome
        .comparison
        .as_ref()
        .is_some_and(|c| c.is_regression());
    Ok(if regressed { REGRESSION_EXIT } else { 0 })
}

/// Runs `fakeland bench` and returns the exit code.
pub async fn run(args: BenchArgs) -> Result<i32> {
    let mut params = files(&args, args.select.bench_params()?)?;
    let startup = params.preset.as_deref() == Some("startup");
    if startup && !args.command.is_empty() && params.params.get("command").is_none() {
        params.params["command"] = serde_json::json!(args.command);
    }
    let session_args = args.session.clone();
    let session = tokio::task::spawn_blocking(move || session_args.start()).await??;
    let controller = Controller::new(session.clone());
    let result = drive(&controller, &args, params, startup).await;
    tokio::task::spawn_blocking(move || session.blocking_shutdown()).await??;
    result
}

async fn drive(
    c: &Controller,
    args: &BenchArgs,
    params: BenchParams,
    startup: bool,
) -> Result<i32> {
    if !startup && let Some(spec) = LaunchSpec::from_argv(args.command.clone()) {
        let info = c
            .launch(LaunchParams {
                spec,
                wait_window: Some(args.wait_window),
            })
            .await?;
        let window = info
            .window
            .context("the app mapped no window in time; raise --wait-window")?;
        eprintln!(
            "fakeland: benching {} ({}, pid {})",
            window.app_id, window.id, info.pid.0
        );
        let _ = c
            .wait_settled(WaitSettledParams {
                window: WindowRef::id(window.id),
                quiet: Dur::from_millis(500),
                timeout: Dur::from_secs(5),
            })
            .await;
        c.session().reset_validity().await?;
    }
    let worker = c.clone();
    let outcome =
        tokio::task::spawn_blocking(move || fakeland::block_on(worker.bench(params))).await??;
    let code = print_outcome(&outcome, args.json)?;
    if !args.json {
        println!("{}", validity_text(&c.session().validity().await?));
    }
    Ok(code)
}
