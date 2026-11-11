//! Bench operations: build a bench from a preset or a scenario, run it, write its files and
//! compare its report with a baseline.

use std::path::Path;

use fakeland_bench::analyzers::{Buffers, FramePacing, InputCoalescing, Latency};
use fakeland_bench::{Bench, BenchReport, PresetParams, compare, presets};
use fakeland_scenario::WindowRef;

use super::capture::write_file;
use super::{Controller, OpError};
use crate::export::export_trace;
use crate::methods::{BenchOutcome, BenchParams, NoParams, PresetEntry, TraceFormat, TraceOutput};

/// Measured iterations of a scenario bench without an explicit count.
const SCENARIO_ITERATIONS: u32 = 5;

/// Returns every preset.
pub fn preset_entries() -> Vec<PresetEntry> {
    presets::list().into_iter().map(PresetEntry::from).collect()
}

/// Builds the bench of the parameters.
pub fn build_bench(p: &BenchParams) -> Result<Bench, OpError> {
    match (&p.preset, &p.scenario) {
        (Some(name), None) => {
            let preset = presets::by_name(name).ok_or_else(|| {
                OpError::NotFound(format!(
                    "no preset {name:?}; presets: {}",
                    presets::NAMES.join(", ")
                ))
            })?;
            let mut params = PresetParams::from_value(p.params.clone()).ok_or_else(|| {
                OpError::InvalidParams("the preset params must be a JSON object".into())
            })?;
            if let Some(n) = p.iterations {
                params = params.set("iterations", n);
            }
            if let Some(n) = p.warmup {
                params = params.set("warmup", n);
            }
            if let Some(t) = &p.target {
                params = params.set("target", t);
            }
            preset
                .bench(&params)
                .map_err(|e| OpError::InvalidParams(e.to_string()))
        }
        (None, Some(scenario)) => {
            if !p.params.is_null() && p.params != serde_json::json!({}) {
                return Err(OpError::InvalidParams(
                    "params apply to presets; a scenario bench takes iterations, warmup and target"
                        .into(),
                ));
            }
            Ok(Bench::new(scenario.name.clone())
                .scenario(scenario.clone())
                .analyze(FramePacing::default())
                .analyze(Latency::default())
                .analyze(InputCoalescing)
                .analyze(Buffers)
                .target(p.target.clone().unwrap_or(WindowRef::Focused))
                .iterations(p.iterations.unwrap_or(SCENARIO_ITERATIONS))
                .warmup(p.warmup.unwrap_or(1)))
        }
        (Some(_), Some(_)) => Err(OpError::InvalidParams(
            "give a preset or a scenario, not both".into(),
        )),
        (None, None) => Err(OpError::InvalidParams("give a preset or a scenario".into())),
    }
}

/// Reads a report file.
pub fn read_report(path: &Path) -> Result<BenchReport, OpError> {
    let text = std::fs::read_to_string(path).map_err(|e| OpError::io(path, e))?;
    BenchReport::from_json(&text)
        .map_err(|e| OpError::InvalidParams(format!("{}: {e}", path.display())))
}

fn check_paths(p: &BenchParams) -> Result<(), OpError> {
    let paths = p
        .traces
        .iter()
        .map(|t| t.path.as_path())
        .chain(p.traces_dir.as_deref())
        .chain(p.report.as_deref())
        .chain(p.baseline.as_deref());
    for path in paths {
        if !path.is_absolute() {
            return Err(OpError::InvalidParams(format!(
                "bench paths must be absolute: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

impl Controller {
    /// Returns every preset.
    pub fn presets(&self, _: NoParams) -> Vec<PresetEntry> {
        preset_entries()
    }

    /// Runs a bench, writes its files and compares it with a baseline.
    pub async fn bench(&self, p: BenchParams) -> Result<BenchOutcome, OpError> {
        check_paths(&p)?;
        let bench = build_bench(&p)?;
        let baseline = p.baseline.as_deref().map(read_report).transpose()?;
        let session = self.session();
        let capture = match p.traces.is_empty() {
            true => None,
            false => Some(session.begin_capture().await?),
        };
        let run = bench.run(session).await;
        let span = match capture {
            Some(id) => Some(session.end_capture(id).await?),
            None => None,
        };
        let run = run.map_err(|e| OpError::Failed(format!("bench failed: {e}")))?;
        let report = run.report();
        let mut files = Vec::new();
        let mut traces = Vec::new();
        if let Some(span) = &span {
            for out in &p.traces {
                traces.push(export_trace(span, out).map_err(|e| OpError::Failed(e.to_string()))?);
            }
        }
        if let Some(dir) = &p.traces_dir {
            for (meta, trace) in run.traces() {
                let kind = if meta.is_warmup() { "warmup" } else { "iter" };
                let out = TraceOutput {
                    path: dir.join(format!("{kind}-{:03}.jsonl", meta.index)),
                    format: Some(TraceFormat::Jsonl),
                };
                export_trace(trace, &out).map_err(|e| OpError::Failed(e.to_string()))?;
                files.push(out.path);
            }
        }
        if let Some(path) = &p.report {
            write_file(path, report.to_json_pretty().as_bytes())?;
            files.push(path.clone());
        }
        let comparison =
            baseline.map(|base| compare(&base, &report, &p.thresholds.clone().unwrap_or_default()));
        Ok(BenchOutcome {
            text: report.to_text(),
            comparison_text: comparison.as_ref().map(|c| c.to_text()),
            comparison,
            report,
            traces,
            files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_scenario::Scenario;

    #[test]
    fn builds_preset_and_scenario_benches() {
        let entries = preset_entries();
        assert_eq!(entries.len(), presets::NAMES.len());
        assert!(entries.iter().all(|e| e.params.is_object()));
        let p = BenchParams {
            preset: Some("pacing".into()),
            iterations: Some(2),
            ..BenchParams::default()
        };
        assert_eq!(build_bench(&p).unwrap().measured_iterations(), 2);
        let p = BenchParams {
            preset: Some("nope".into()),
            ..BenchParams::default()
        };
        assert!(matches!(build_bench(&p), Err(OpError::NotFound(_))));
        let p = BenchParams {
            preset: Some("pacing".into()),
            params: serde_json::json!({"no_such_param": 1}),
            ..BenchParams::default()
        };
        assert!(matches!(build_bench(&p), Err(OpError::InvalidParams(_))));
        let scenario = Scenario::builder("s").mark("x").build().unwrap();
        let p = BenchParams {
            scenario: Some(scenario),
            ..BenchParams::default()
        };
        let b = build_bench(&p).unwrap();
        assert_eq!(b.measured_iterations(), SCENARIO_ITERATIONS);
        assert_eq!(b.analyzer_names().len(), 4);
        assert!(build_bench(&BenchParams::default()).is_err());
    }

    #[test]
    fn rejects_relative_paths() {
        let p = BenchParams {
            preset: Some("pacing".into()),
            report: Some("out.json".into()),
            ..BenchParams::default()
        };
        assert!(check_paths(&p).is_err());
    }
}
