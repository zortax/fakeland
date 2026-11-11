//! `fakeland export`: convert a trace file into other formats and print its summary.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use fakeland_control::export::{export_trace, read_trace, trace_info};
use fakeland_control::methods::{TraceFormat, TraceOutput};
use fakeland_control::text::summary_text;

use crate::util::{absolute, print_json};

/// Flags of `fakeland export`.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// The trace file, JSONL or JSON.
    input: PathBuf,
    /// Write a Chrome trace for ui.perfetto.dev.
    #[arg(long, value_name = "FILE")]
    perfetto: Option<PathBuf>,
    /// Write one CSV row per frame.
    #[arg(long, value_name = "FILE")]
    csv: Option<PathBuf>,
    /// Write JSON lines.
    #[arg(long, value_name = "FILE")]
    jsonl: Option<PathBuf>,
    /// Write one JSON document.
    #[arg(long, value_name = "FILE")]
    json: Option<PathBuf>,
    /// Write the JSON summary.
    #[arg(long, value_name = "FILE")]
    summary: Option<PathBuf>,
    /// Keep only the records of one window and its subsurfaces and popups.
    #[arg(long, value_name = "ID")]
    window: Option<u64>,
    /// Print the summary as JSON.
    #[arg(long = "print-json")]
    print_json: bool,
}

/// Runs `fakeland export`.
pub fn run(args: ExportArgs) -> Result<i32> {
    let mut trace = read_trace(&args.input)?;
    if let Some(w) = args.window {
        trace = trace.for_window(fakeland_types::WindowId(w));
    }
    let targets = [
        (&args.perfetto, TraceFormat::Perfetto),
        (&args.csv, TraceFormat::Csv),
        (&args.jsonl, TraceFormat::Jsonl),
        (&args.json, TraceFormat::Json),
        (&args.summary, TraceFormat::Summary),
    ];
    let mut wrote = false;
    for (path, format) in targets {
        if let Some(p) = path {
            let out = TraceOutput {
                path: absolute(p)?,
                format: Some(format),
            };
            export_trace(&trace, &out)?;
            eprintln!("fakeland: {} -> {}", format.as_str(), out.path.display());
            wrote = true;
        }
    }
    let info = trace_info(&trace);
    if args.print_json {
        print_json(&info)?;
    } else {
        print!("{}", summary_text(&info.summary));
    }
    if !wrote && !args.print_json && trace.records.is_empty() {
        bail!("{} holds no records", args.input.display());
    }
    Ok(0)
}
