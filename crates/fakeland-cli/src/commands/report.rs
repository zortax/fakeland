//! `fakeland report`, `fakeland compare` and `fakeland presets`: read and compare bench reports
//! and list the presets.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use fakeland_bench::{Thresholds, compare};
use fakeland_control::ops::{preset_entries, read_report};

use super::bench::REGRESSION_EXIT;
use crate::util::print_json;

/// Flags of `fakeland report`.
#[derive(Debug, Args)]
pub struct ReportArgs {
    /// The report JSON file.
    file: PathBuf,
}

/// Flags of `fakeland compare`.
#[derive(Debug, Args)]
pub struct CompareArgs {
    /// The baseline report.
    baseline: PathBuf,
    /// The current report.
    current: PathBuf,
    /// A change must exceed this fraction of the baseline to count.
    #[arg(long, default_value_t = 0.05)]
    relative: f64,
    /// A change must also exceed this absolute amount.
    #[arg(long, default_value_t = 0.0)]
    absolute: f64,
    /// Count changes even when the confidence intervals overlap.
    #[arg(long)]
    no_ci: bool,
    /// Print the comparison as JSON.
    #[arg(long)]
    json: bool,
}

/// Flags of `fakeland presets`.
#[derive(Debug, Args)]
pub struct PresetsArgs {
    /// Show the parameter schema and defaults of this preset.
    name: Option<String>,
    /// Print as JSON.
    #[arg(long)]
    json: bool,
}

/// Runs `fakeland report`.
pub fn report(args: ReportArgs) -> Result<i32> {
    print!("{}", read_report(&args.file)?.to_text());
    Ok(0)
}

/// Runs `fakeland compare` and returns 3 on a regression.
pub fn compare_reports(args: CompareArgs) -> Result<i32> {
    let base = read_report(&args.baseline)?;
    let current = read_report(&args.current)?;
    let thresholds = Thresholds {
        relative: args.relative,
        absolute: args.absolute,
        require_ci_separation: !args.no_ci,
        ..Thresholds::default()
    };
    let c = compare(&base, &current, &thresholds);
    if args.json {
        print_json(&c)?;
    } else {
        print!("{}", c.to_text());
    }
    Ok(if c.is_regression() {
        REGRESSION_EXIT
    } else {
        0
    })
}

/// Runs `fakeland presets`.
pub fn presets(args: PresetsArgs) -> Result<i32> {
    let entries = preset_entries();
    match &args.name {
        None if args.json => print_json(&entries)?,
        None => {
            let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
            for e in &entries {
                println!("{:<width$}  {}", e.name, e.description);
            }
            println!("\nShow parameters with `fakeland presets NAME`.");
        }
        Some(name) => {
            let Some(e) = entries.iter().find(|e| &e.name == name) else {
                bail!("no preset {name:?}");
            };
            if args.json {
                print_json(e)?;
            } else {
                println!("{}\n\n{}\n\ndefaults:", e.name, e.description);
                println!("{}", serde_json::to_string_pretty(&e.defaults)?);
                println!("\nschema:\n{}", serde_json::to_string_pretty(&e.params)?);
            }
        }
    }
    Ok(0)
}
