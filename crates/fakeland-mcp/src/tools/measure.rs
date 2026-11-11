//! Scenario, latency, frame statistics and trace tools.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use fakeland_control::Controller;
use fakeland_control::methods::{
    BenchParams, CancelRunParams, FrameStatsParams as CtlStats, NoParams,
    RunScenarioParams as CtlRun, TraceEndParams, TraceFormat, TraceOutput,
};
use fakeland_control::text::{probe_text, run_text, windows_text};
use fakeland_scenario::{Action, Combo, Detector, KeyAction, PointerAction, ProbeSpec, Scenario};
use fakeland_types::Dur;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use super::common::{limit, matcher, ms};
use super::screen::roi;
use crate::params::{
    BenchmarkParams, FrameStatsParams, LatencyAction, LatencyDetector, MeasureLatencyParams,
    RunScenarioParams, TraceStartParams, TraceStopParams,
};
use crate::reply::{ToolResult, done, done_json, finish};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

fn absolute(path: &str, what: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        bail!("{what} must be an absolute path: {path}");
    }
    Ok(p)
}

impl FakelandMcp {
    async fn probe_spec(&self, c: &Controller, p: &MeasureLatencyParams) -> Result<ProbeSpec> {
        if let Some(v) = &p.probe {
            return serde_json::from_value(v.clone()).context("probe is no valid probe spec");
        }
        let action = p
            .action
            .context("give `probe`, or `action` with x, y and roi")?;
        let window = matcher(p.window.as_deref())?;
        let pos = |x: f64, y: f64| self.state.position(c, x, y, p.logical, p.window.as_deref());
        let (input, alternate) = match action {
            LatencyAction::Hover => {
                let (here, _) = pos(p.x, p.y).await?;
                let away = match p.away {
                    Some([x, y]) => Some(Action::from(PointerAction::move_to(pos(x, y).await?.0))),
                    None => None,
                };
                (Action::from(PointerAction::move_to(here)), away)
            }
            LatencyAction::Click => {
                let (here, _) = pos(p.x, p.y).await?;
                (
                    Action::from(PointerAction::click(
                        Some(here),
                        fakeland_scenario::Button::Left,
                    )),
                    None,
                )
            }
            LatencyAction::Key => {
                let key = p.key.as_deref().context("the key action needs `key`")?;
                let combo = Combo::parse(key).with_context(|| format!("bad key {key:?}"))?;
                (Action::from(KeyAction::combo(combo)), None)
            }
        };
        let region = roi(window, p.roi);
        let detector = match p.detector {
            LatencyDetector::PixelsChanged => Detector::PixelsChanged {
                roi: region,
                threshold: 0.0,
            },
            LatencyDetector::AnyDamage => Detector::AnyDamage { roi: region },
            LatencyDetector::AppMark => Detector::AppMark { name: None },
        };
        let mut spec = ProbeSpec::new(input, detector)
            .repeat(p.repeat.clamp(1, 1000))
            .label("measure_latency");
        if let Some(a) = alternate {
            spec = spec.alternate(a);
        }
        Ok(spec)
    }
}

#[tool_router(router = measure_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Runs a scenario.
    #[tool(
        description = "Run a scenario on the compositor: timed steps (input, waits, captures, marks, window and output actions) and triggers, with exact compositor timing. Returns per-step times, probe results and errors. Call scenario_schema for the format."
    )]
    async fn run_scenario(&self, Parameters(p): Parameters<RunScenarioParams>) -> ToolResult {
        finish(
            async {
                let scenario = Scenario::from_json(&p.scenario.to_string())
                    .map_err(|e| anyhow::anyhow!("invalid scenario: {e}"))?;
                let c = self.ctl().await?;
                let params = CtlRun {
                    scenario,
                    frames: Vec::new(),
                    captures_dir: p
                        .captures_dir
                        .as_deref()
                        .map(|d| absolute(d, "captures_dir"))
                        .transpose()?,
                    trace: p
                        .trace_path
                        .as_deref()
                        .map(|t| absolute(t, "trace_path"))
                        .transpose()?
                        .map(|path| TraceOutput { path, format: None }),
                };
                let started = bounded(CALL_SLACK, c.run_scenario(params, None)).await??;
                if !p.wait {
                    return done(format!("Started run {}.", started.run.0));
                }
                let limit = Duration::from_millis(p.timeout_ms);
                match tokio::time::timeout(limit, c.wait_run(started.run)).await {
                    Ok(summary) => {
                        let summary = summary?;
                        done_json(run_text(&summary), &summary)
                    }
                    Err(_) => {
                        let _ = c.cancel_run(CancelRunParams { run: started.run }).await;
                        bail!(
                            "run {} took longer than {} ms and was cancelled",
                            started.run.0,
                            p.timeout_ms
                        )
                    }
                }
            }
            .await,
        )
    }

    /// Returns the scenario schema.
    #[tool(
        description = "Return the JSON schema of scenarios for run_scenario, with every action, anchor, wait and trigger."
    )]
    async fn scenario_schema(&self, Parameters(_): Parameters<TraceStartParams>) -> ToolResult {
        finish(
            serde_json::to_string(&Scenario::json_schema())
                .map_err(anyhow::Error::from)
                .and_then(done),
        )
    }

    /// Measures input latency.
    #[tool(
        description = "Measure input-to-screen latency with compositor timestamps. Simple form: action hover|click|key at x, y (screenshot pixels by default) and a roi that must respond (logical, relative to window or output); hover with away toggles between two points. Returns per-repeat stages input>commit>ready>latch>present and percentiles."
    )]
    async fn measure_latency(&self, Parameters(p): Parameters<MeasureLatencyParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let spec = self.probe_spec(&c, &p).await?;
                let per_repeat = spec.timeout.0.as_millis_f64() + spec.settle.0.as_millis_f64();
                let total = (f64::from(spec.repeat) * (per_repeat + 1500.0)) as u64;
                let report = bounded(limit(total), c.probe(spec)).await??;
                done_json(probe_text(&report), &report)
            }
            .await,
        )
    }

    /// Summarizes recent frames.
    #[tool(
        description = "Summarize the recent frames of a window: fps, present intervals, client, GPU and queue times, missed vblanks, buffer formats and input latency, from compositor timestamps."
    )]
    async fn frame_stats(&self, Parameters(p): Parameters<FrameStatsParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let stats = bounded(
                    CALL_SLACK,
                    c.frame_stats(CtlStats {
                        window: fakeland_control::spec::parse_window_ref(&p.window)?,
                        last: Dur(ms(p.last_ms.max(1))),
                    }),
                )
                .await??;
                done(format!(
                    "{}{}",
                    windows_text(std::slice::from_ref(&stats.window)),
                    stats.text
                ))
            }
            .await,
        )
    }

    /// Starts a trace capture.
    #[tool(
        description = "Start recording a trace of every frame, input and compositor event. Stop it with trace_stop."
    )]
    async fn trace_start(&self, Parameters(_): Parameters<TraceStartParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let cap = bounded(CALL_SLACK, c.trace_begin(NoParams {})).await??;
                done(format!("Trace capture {} started.", cap.capture))
            }
            .await,
        )
    }

    /// Runs a benchmark preset.
    #[tool(
        description = "Run a benchmark preset against a window and return its report: pacing, hover-latency, click-latency, key-latency, typing, scroll-glide, fling, drag-follow, resize-storm, startup, popup-open, throughput, idle, refresh-sweep, scale-switch. preset=list shows every preset with its parameters. Benches take seconds to minutes."
    )]
    async fn run_benchmark(&self, Parameters(p): Parameters<BenchmarkParams>) -> ToolResult {
        finish(
            async {
                if p.preset == "list" {
                    let list = fakeland_control::ops::preset_entries();
                    let names: Vec<String> = list
                        .iter()
                        .map(|e| format!("{}: {}", e.name, e.description))
                        .collect();
                    return done_json(names.join("\n"), &list);
                }
                let c = self.ctl().await?;
                let mut params = BenchParams {
                    preset: Some(p.preset.clone()),
                    params: p.params.clone(),
                    iterations: p.iterations,
                    target: matcher(p.window.as_deref())?,
                    report: p
                        .report_path
                        .as_deref()
                        .map(|r| absolute(r, "report_path"))
                        .transpose()?,
                    ..BenchParams::default()
                };
                if let Some(path) = &p.perfetto_path {
                    params.traces.push(TraceOutput {
                        path: absolute(path, "perfetto_path")?,
                        format: Some(TraceFormat::Perfetto),
                    });
                }
                let outcome =
                    tokio::task::spawn_blocking(move || fakeland::block_on(c.bench(params)))
                        .await
                        .context("the bench thread failed")??;
                done_json(outcome.text.clone(), &outcome.report)
            }
            .await,
        )
    }

    /// Stops the trace capture and writes it.
    #[tool(
        description = "Stop the newest trace recording, write it to path and return its frame summary. A path ending in .perfetto.json opens in ui.perfetto.dev."
    )]
    async fn trace_stop(&self, Parameters(p): Parameters<TraceStopParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let path = absolute(&p.path, "path")?;
                let info = bounded(
                    limit(10_000),
                    c.trace_end(TraceEndParams {
                        capture: None,
                        output: Some(TraceOutput { path, format: None }),
                    }),
                )
                .await??;
                done(format!(
                    "Wrote {} records ({} format) to {}\n{}",
                    info.records,
                    info.format.map_or("-", |f| f.as_str()),
                    p.path,
                    fakeland_control::text::summary_text(&info.summary)
                ))
            }
            .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_absolute_paths() {
        assert!(absolute("/tmp/x.jsonl", "path").is_ok());
        assert!(absolute("x.jsonl", "path").is_err());
    }
}
