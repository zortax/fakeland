//! Scenario runs: start, track, summarize, cancel. A helper thread waits for each run, writes its
//! files, stores its summary and calls the notify hook of the caller.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fakeland::{RunHandle, RunOptions, RunResult};
use fakeland_scenario::Scenario;
use fakeland_types::RunId;
use futures_channel::oneshot;

use super::capture::{shot_info, shot_meta, write_file};
use super::timer::with_timeout;
use super::{Controller, OpError};
use crate::export::export_trace;
use crate::methods::{
    CancelRunParams, NoParams, RunCapture, RunFrames, RunResultParams, RunScenarioParams,
    RunStarted, RunState, RunSummary, TraceOutput,
};

/// Finished runs that the registry keeps.
const KEEP_FINISHED: usize = 64;

/// A hook that receives the summary of a finished run.
pub type RunNotify = Box<dyn FnOnce(&RunSummary) + Send>;

#[derive(Debug)]
struct RunEntry {
    name: String,
    summary: Option<RunSummary>,
    waiters: Vec<oneshot::Sender<RunSummary>>,
}

/// Every run the controller started, by id.
#[derive(Debug, Default)]
pub struct RunRegistry {
    runs: BTreeMap<u64, RunEntry>,
}

impl RunRegistry {
    fn start(&mut self, run: RunId, name: String) {
        self.runs.insert(
            run.0,
            RunEntry {
                name,
                summary: None,
                waiters: Vec::new(),
            },
        );
    }

    fn finish(&mut self, summary: RunSummary) {
        if let Some(entry) = self.runs.get_mut(&summary.run.0) {
            for w in entry.waiters.drain(..) {
                let _ = w.send(summary.clone());
            }
            entry.summary = Some(summary);
        }
        let finished: Vec<u64> = self
            .runs
            .iter()
            .filter(|(_, e)| e.summary.is_some())
            .map(|(k, _)| *k)
            .collect();
        for k in finished
            .iter()
            .take(finished.len().saturating_sub(KEEP_FINISHED))
        {
            self.runs.remove(k);
        }
    }

    fn state(&self, run: RunId) -> Option<RunState> {
        self.runs.get(&run.0).map(|e| RunState {
            run,
            name: e.name.clone(),
            summary: e.summary.clone(),
        })
    }

    fn waiter(&mut self, run: RunId) -> Option<oneshot::Receiver<RunSummary>> {
        let entry = self.runs.get_mut(&run.0)?;
        let (tx, rx) = oneshot::channel();
        match &entry.summary {
            Some(s) => {
                let _ = tx.send(s.clone());
            }
            None => entry.waiters.push(tx),
        }
        Some(rx)
    }
}

/// Where the files of a finished run go.
#[derive(Clone, Debug, Default)]
struct RunFiles {
    captures_dir: Option<PathBuf>,
    trace: Option<TraceOutput>,
}

fn check_absolute(path: &Path, what: &str) -> Result<(), OpError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(OpError::InvalidParams(format!(
            "the {what} path must be absolute: {}",
            path.display()
        )))
    }
}

impl Controller {
    fn runs(&self) -> std::sync::MutexGuard<'_, RunRegistry> {
        self.inner.runs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a run and returns its id at once. `notify` receives the summary when it ends.
    pub async fn run_scenario(
        &self,
        p: RunScenarioParams,
        notify: Option<RunNotify>,
    ) -> Result<RunStarted, OpError> {
        if let Some(dir) = &p.captures_dir {
            check_absolute(dir, "captures_dir")?;
        }
        if let Some(t) = &p.trace {
            check_absolute(&t.path, "trace")?;
        }
        p.scenario
            .validate()
            .map_err(|e| OpError::InvalidParams(e.to_string()))?;
        let files = RunFiles {
            captures_dir: p.captures_dir,
            trace: p.trace,
        };
        self.start_run(p.scenario, RunOptions { frames: p.frames }, files, notify)
            .await
    }

    async fn start_run(
        &self,
        scenario: Scenario,
        options: RunOptions,
        files: RunFiles,
        notify: Option<RunNotify>,
    ) -> Result<RunStarted, OpError> {
        let name = scenario.name.clone();
        let handle = self.session().run_with(scenario, options).await?;
        let run = handle.id();
        self.runs().start(run, name);
        let registry = Arc::clone(&self.inner.runs);
        std::thread::Builder::new()
            .name(format!("fakeland-run-{}", run.0))
            .spawn(move || finish_run(handle, files, &registry, notify))
            .map_err(|e| OpError::Failed(format!("cannot start the run thread: {e}")))?;
        Ok(RunStarted { run })
    }

    /// Starts a run without files and returns its id.
    pub(super) async fn start_plain_run(&self, scenario: Scenario) -> Result<RunId, OpError> {
        Ok(self
            .start_run(scenario, RunOptions::default(), RunFiles::default(), None)
            .await?
            .run)
    }

    /// Returns the state of a run, after a wait for its end when asked.
    pub async fn run_result(&self, p: RunResultParams) -> Result<RunState, OpError> {
        let not_found = || OpError::NotFound(format!("no run {}", p.run.0));
        let state = self.runs().state(p.run).ok_or_else(not_found)?;
        let Some(wait) = p.wait.filter(|_| state.summary.is_none()) else {
            return Ok(state);
        };
        let rx = self.runs().waiter(p.run).ok_or_else(not_found)?;
        match with_timeout(rx, wait.0).await {
            Some(Ok(summary)) => Ok(RunState {
                run: p.run,
                name: summary.name.clone(),
                summary: Some(summary),
            }),
            Some(Err(_)) => Err(OpError::Failed("the run thread ended".into())),
            None => Ok(state),
        }
    }

    /// Waits for the summary of a run.
    pub async fn wait_run(&self, run: RunId) -> Result<RunSummary, OpError> {
        let rx = self
            .runs()
            .waiter(run)
            .ok_or_else(|| OpError::NotFound(format!("no run {}", run.0)))?;
        rx.await
            .map_err(|_| OpError::Failed("the run thread ended".into()))
    }

    /// Cancels a run.
    pub async fn cancel_run(&self, p: CancelRunParams) -> Result<bool, OpError> {
        Ok(self.session().cancel(p.run).await?)
    }

    /// Returns every known run.
    pub fn runs_list(&self, _: NoParams) -> Vec<RunState> {
        let reg = self.runs();
        reg.runs
            .keys()
            .filter_map(|k| reg.state(RunId(*k)))
            .collect()
    }
}

fn finish_run(
    handle: RunHandle,
    files: RunFiles,
    registry: &Mutex<RunRegistry>,
    notify: Option<RunNotify>,
) {
    let run = handle.id();
    let summary = match handle.blocking_wait() {
        Ok(result) => summarize(&result, &RunFilesRef::from(&files)),
        Err(e) => closed_summary(run, &e.to_string()),
    };
    registry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .finish(summary.clone());
    if let Some(notify) = notify {
        notify(&summary);
    }
}

fn closed_summary(run: RunId, error: &str) -> RunSummary {
    RunSummary {
        run,
        name: String::new(),
        ok: false,
        error: Some(error.to_owned()),
        error_kind: Some(fakeland::RunErrorKind::Closed),
        started: fakeland_types::Timestamp(0),
        finished: fakeland_types::Timestamp(0),
        duration_ms: 0.0,
        steps: Vec::new(),
        probes: Vec::new(),
        triggers: Vec::new(),
        captures: Vec::new(),
        frames: Vec::new(),
        trace_records: 0,
        trace: None,
        file_errors: Vec::new(),
    }
}

/// Builds the summary of a run and writes its files.
pub fn summarize(result: &RunResult, files: &RunFilesRef<'_>) -> RunSummary {
    let mut errors = Vec::new();
    let captures = result
        .captures
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let path = files
                .captures_dir
                .map(|d| d.join(capture_name(i, c.label.as_deref())));
            let shot = shot_info(&c.screenshot, path.as_deref(), false).unwrap_or_else(|e| {
                errors.push(e.to_string());
                shot_meta(&c.screenshot)
            });
            RunCapture {
                step: c.path.clone(),
                label: c.label.clone(),
                shot,
            }
        })
        .collect();
    let frames = result
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let dir = files.captures_dir.map(|d| d.join(format!("frames-{i}")));
            if let Some(dir) = &dir {
                for (n, frame) in f.frames.iter().enumerate() {
                    let written = frame
                        .image
                        .encode_png()
                        .map_err(|e| OpError::Failed(e.to_string()))
                        .and_then(|png| write_file(&dir.join(format!("{n:05}.png")), &png));
                    if let Err(e) = written {
                        errors.push(e.to_string());
                        break;
                    }
                }
            }
            RunFrames {
                spec: f.spec.clone(),
                frames: f.frames.len(),
                dropped: f.dropped,
                cost_ms: f.cost.as_millis_f64(),
                dir,
            }
        })
        .collect();
    let trace = files.trace.and_then(|out| {
        export_trace(&result.trace, out)
            .map_err(|e| errors.push(e.to_string()))
            .ok()
    });
    RunSummary {
        run: result.run,
        name: result.name.clone(),
        ok: result.ok,
        error: result.error.clone(),
        error_kind: result.error_kind,
        started: result.started,
        finished: result.finished,
        duration_ms: result.duration().as_millis_f64(),
        steps: result.steps.clone(),
        probes: result.probes.clone(),
        triggers: result.triggers.clone(),
        captures,
        frames,
        trace_records: result.trace.records.len(),
        trace,
        file_errors: errors,
    }
}

/// Borrowed file destinations of a run summary.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunFilesRef<'a> {
    /// The directory for PNG files.
    pub captures_dir: Option<&'a Path>,
    /// The trace file.
    pub trace: Option<&'a TraceOutput>,
}

impl<'a> From<&'a RunFiles> for RunFilesRef<'a> {
    fn from(f: &'a RunFiles) -> Self {
        Self {
            captures_dir: f.captures_dir.as_deref(),
            trace: f.trace.as_ref(),
        }
    }
}

/// Returns the file name of a capture step: its label, made safe, or its index.
fn capture_name(index: usize, label: Option<&str>) -> String {
    let safe: String = label
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches('.');
    if safe.is_empty() {
        format!("capture-{index:03}.png")
    } else {
        format!("{index:03}-{safe}.png")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_captures_safely() {
        assert_eq!(capture_name(2, None), "capture-002.png");
        assert_eq!(capture_name(1, Some("after click")), "001-after_click.png");
        assert_eq!(capture_name(0, Some("../x")), "000-_x.png");
    }

    #[test]
    fn registry_keeps_and_wakes() {
        let mut reg = RunRegistry::default();
        reg.start(RunId(1), "a".into());
        let mut rx = reg.waiter(RunId(1)).unwrap();
        assert!(reg.state(RunId(1)).unwrap().summary.is_none());
        reg.finish(closed_summary(RunId(1), "x"));
        assert!(rx.try_recv().unwrap().is_some());
        assert!(reg.state(RunId(1)).unwrap().summary.is_some());
        for i in 2..(KEEP_FINISHED as u64 + 10) {
            reg.start(RunId(i), "b".into());
            reg.finish(closed_summary(RunId(i), "x"));
        }
        assert_eq!(reg.runs.len(), KEEP_FINISHED);
        assert!(reg.state(RunId(1)).is_none());
    }
}
