//! Helpers for `harness = false` bench targets: a name filter from the command line, report
//! files, and a comparison with a saved baseline.
//!
//! `cargo bench -p my-crate -- hover` runs the benches whose name contains `hover`. Each report
//! lands in `target/fakeland-bench/<name>.json` with a text copy. With
//! `FAKELAND_BENCH_SAVE_BASELINE=1` the report also becomes the baseline; otherwise an existing
//! baseline is compared against it.

use std::path::{Path, PathBuf};

use fakeland::Session;

use crate::compare::{Comparison, Thresholds, compare};
use crate::report::BenchReport;
use crate::runner::Bench;

/// Runs benches by name, writes their reports and compares them with baselines.
#[derive(Debug)]
pub struct Harness {
    filters: Vec<String>,
    out_dir: PathBuf,
    save_baseline: bool,
    thresholds: Thresholds,
    regressions: Vec<String>,
}

impl Harness {
    /// Builds a harness from the command line and the environment. Arguments that start with
    /// `-` are cargo flags and do not filter. `FAKELAND_BENCH_OUT` sets the output directory.
    pub fn from_env() -> Self {
        let filters = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let out_dir = std::env::var_os("FAKELAND_BENCH_OUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/fakeland-bench"));
        Self {
            filters,
            out_dir,
            save_baseline: std::env::var_os("FAKELAND_BENCH_SAVE_BASELINE").is_some(),
            thresholds: Thresholds::default(),
            regressions: Vec::new(),
        }
    }

    /// Sets the output directory unless `FAKELAND_BENCH_OUT` sets it. Bench targets run in their
    /// package directory, so they pass the workspace target directory here.
    pub fn default_out_dir(self, dir: impl Into<PathBuf>) -> Self {
        if std::env::var_os("FAKELAND_BENCH_OUT").is_some() {
            self
        } else {
            self.out_dir(dir)
        }
    }

    /// Sets the output directory.
    pub fn out_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.out_dir = dir.into();
        self
    }

    /// Saves each report as the baseline; a harness that saves baselines skips the comparison.
    pub fn save_baseline(mut self, save: bool) -> Self {
        self.save_baseline = save;
        self
    }

    /// Sets the regression thresholds.
    pub fn thresholds(mut self, thresholds: Thresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Returns true when the filter selects a bench name.
    pub fn enabled(&self, name: &str) -> bool {
        self.filters.is_empty() || self.filters.iter().any(|f| name.contains(f.as_str()))
    }

    /// Runs a bench when the filter selects it, prints its report, writes it and compares it with
    /// the baseline. Returns the report, or `None` when the filter skips the bench or it fails.
    pub fn run(&mut self, name: &str, bench: &Bench, session: &Session) -> Option<BenchReport> {
        if !self.enabled(name) {
            return None;
        }
        eprintln!("running {name}");
        let report = match bench.blocking_run(session) {
            Ok(run) => run.into_report(),
            Err(e) => {
                eprintln!("{name}: {e}");
                self.regressions.push(format!("{name}: {e}"));
                return None;
            }
        };
        println!("{}", report.to_text());
        if let Some(c) = self.record(name, &report) {
            println!("{}", c.to_text());
        }
        Some(report)
    }

    /// Writes a report, saves it as the baseline when the harness saves baselines, and else
    /// compares it with the saved baseline. A regression counts toward [`Harness::finish`].
    pub fn record(&mut self, name: &str, report: &BenchReport) -> Option<Comparison> {
        if let Err(e) = self.write(name, report) {
            eprintln!("{name}: cannot write the report: {e}");
        }
        let c = self.compare(name, report)?;
        if c.is_regression() {
            self.regressions.push(name.to_string());
        }
        Some(c)
    }

    fn write(&self, name: &str, report: &BenchReport) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.out_dir)?;
        std::fs::write(
            self.out_dir.join(format!("{name}.json")),
            report.to_json_pretty(),
        )?;
        std::fs::write(self.out_dir.join(format!("{name}.txt")), report.to_text())?;
        if self.save_baseline {
            let dir = self.baseline_dir();
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join(format!("{name}.json")), report.to_json_pretty())?;
        }
        Ok(())
    }

    fn baseline_dir(&self) -> PathBuf {
        self.out_dir.join("baseline")
    }

    fn compare(&self, name: &str, report: &BenchReport) -> Option<Comparison> {
        if self.save_baseline {
            return None;
        }
        let base = read(&self.baseline_dir().join(format!("{name}.json")))?;
        Some(compare(&base, report, &self.thresholds))
    }

    /// Returns the benches that regressed or failed so far.
    pub fn regressions(&self) -> &[String] {
        &self.regressions
    }

    /// Prints a summary and returns the process exit code: 1 after a regression or failure.
    pub fn finish(self) -> std::process::ExitCode {
        if self.regressions.is_empty() {
            std::process::ExitCode::SUCCESS
        } else {
            eprintln!("regressed or failed: {}", self.regressions.join(", "));
            std::process::ExitCode::FAILURE
        }
    }
}

fn read(path: &Path) -> Option<BenchReport> {
    let text = std::fs::read_to_string(path).ok()?;
    BenchReport::from_json(&text).ok()
}
