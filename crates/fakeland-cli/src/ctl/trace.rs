//! Wait, run, probe, trigger, trace, statistics and subscription commands of ctl.

use anyhow::{Context, Result};
use fakeland_control::methods::*;
use fakeland_control::spec::parse_window_ref;
use fakeland_control::text::{probe_text, run_text, windows_text};
use fakeland_control::{ControlClient, SubEvent};
use fakeland_scenario::{ProbeSpec, Scenario, Trigger};
use fakeland_types::RunId;

use super::args::{CtlCommand, TraceCommand, TriggersCommand, WaitCommand};
use super::print::emit;
use crate::util::{absolute, print_json};

fn read(path: &std::path::Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

fn file_info(i: &TraceFileInfo) -> String {
    let path = i
        .path
        .as_ref()
        .map_or_else(String::new, |p| format!(" -> {}", p.display()));
    format!(
        "{} records over {:.1} ms{path}\n{}",
        i.records,
        i.duration_ms,
        fakeland_control::text::summary_text(&i.summary)
    )
}

fn parse_triggers(text: &str) -> Result<Vec<Trigger>> {
    #[derive(serde::Deserialize)]
    struct Set {
        triggers: Vec<Trigger>,
    }
    serde_json::from_str::<Vec<Trigger>>(text)
        .or_else(|_| serde_json::from_str::<Set>(text).map(|s| s.triggers))
        .context("expected an array of triggers or {\"triggers\": [...]}")
}

/// Runs a command of this group. Returns `None` for commands of other groups.
pub fn run(c: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    match cmd {
        CtlCommand::Wait(w) => wait(c, json, w)?,
        CtlCommand::Run {
            file,
            trace,
            captures,
            detach,
        } => {
            let scenario = Scenario::from_json(&read(file)?)
                .with_context(|| format!("{} is no valid scenario", file.display()))?;
            let params = RunScenarioParams {
                scenario,
                frames: Vec::new(),
                captures_dir: captures.as_deref().map(absolute).transpose()?,
                trace: trace
                    .as_deref()
                    .map(absolute)
                    .transpose()?
                    .map(|path| TraceOutput { path, format: None }),
            };
            if *detach {
                let started = c.blocking_call::<RunScenario>(params)?;
                emit(json, &started, |s| format!("run {}", s.run.0))?;
                return Ok(Some(0));
            }
            let summary = c.blocking_run_and_wait(params)?;
            emit(json, &summary, run_text)?;
            return Ok(Some(if summary.ok { 0 } else { 1 }));
        }
        CtlCommand::RunResult { run, wait } => {
            let state = c.blocking_call::<RunResultOf>(RunResultParams {
                run: RunId(*run),
                wait: *wait,
            })?;
            emit(json, &state, |s| match &s.summary {
                Some(summary) => run_text(summary),
                None => format!("run {} {:?} is running", s.run.0, s.name),
            })?;
        }
        CtlCommand::Probe { file } => {
            let spec: ProbeSpec = serde_json::from_str(&read(file)?)
                .with_context(|| format!("{} is no probe spec", file.display()))?;
            let report = c.blocking_call::<Probe>(spec)?;
            emit(json, &report, probe_text)?;
        }
        CtlCommand::Triggers(TriggersCommand::Install { file, lifetime }) => {
            let set = c.blocking_call::<InstallTriggers>(InstallTriggersParams {
                triggers: parse_triggers(&read(file)?)?,
                lifetime: *lifetime,
                name: file.file_stem().map(|s| s.to_string_lossy().into_owned()),
            })?;
            emit(json, &set, |s| format!("trigger set {}", s.set.0))?;
        }
        CtlCommand::Triggers(TriggersCommand::Remove { set }) => {
            let firings =
                c.blocking_call::<RemoveTriggers>(RemoveTriggersParams { set: RunId(*set) })?;
            emit(json, &firings, |f| format!("{} firings", f.len()))?;
        }
        CtlCommand::Trace(TraceCommand::Start) => {
            let cap = c.blocking_call::<TraceBegin>(NoParams {})?;
            emit(json, &cap, |c| format!("trace capture {}", c.capture))?;
        }
        CtlCommand::Trace(TraceCommand::Stop { file, id }) => {
            let info = c.blocking_call::<TraceEnd>(TraceEndParams {
                capture: *id,
                output: Some(TraceOutput {
                    path: absolute(file)?,
                    format: None,
                }),
            })?;
            emit(json, &info, file_info)?;
        }
        CtlCommand::Trace(TraceCommand::Snapshot { file, last }) => {
            let info = c.blocking_call::<TraceSnapshot>(TraceSnapshotParams {
                last: *last,
                output: Some(TraceOutput {
                    path: absolute(file)?,
                    format: None,
                }),
            })?;
            emit(json, &info, file_info)?;
        }
        CtlCommand::Stats { window, last } => {
            let stats = c.blocking_call::<FrameStatsOf>(FrameStatsParams {
                window: parse_window_ref(window)?,
                last: *last,
            })?;
            emit(json, &stats, |s| {
                format!(
                    "{}{}",
                    windows_text(std::slice::from_ref(&s.window)),
                    s.text
                )
            })?;
        }
        CtlCommand::Subscribe { types } => {
            let mut sub = c.blocking_subscribe(SubscribeParams {
                types: types.clone(),
                ..SubscribeParams::default()
            })?;
            while let Some(event) = sub.blocking_next() {
                match event {
                    SubEvent::Record(r) => println!("{}", serde_json::to_string(&r)?),
                    SubEvent::Gap { missed } => eprintln!("fakeland: {missed} records dropped"),
                }
            }
        }
        _ => return Ok(None),
    }
    Ok(Some(0))
}

fn wait(c: &ControlClient, json: bool, w: &WaitCommand) -> Result<()> {
    match w {
        WaitCommand::Window { window, timeout } => {
            let info = c.blocking_call::<WaitWindow>(WaitWindowParams {
                window: parse_window_ref(window)?,
                timeout: *timeout,
            })?;
            emit(json, &info, |w| windows_text(std::slice::from_ref(w)))
        }
        WaitCommand::Settled {
            window,
            quiet,
            timeout,
        } => {
            let info = c.blocking_call::<WaitSettled>(WaitSettledParams {
                window: parse_window_ref(window)?,
                quiet: *quiet,
                timeout: *timeout,
            })?;
            emit(json, &info, |w| windows_text(std::slice::from_ref(w)))
        }
        WaitCommand::Frames { n, window, timeout } => {
            let t = c.blocking_call::<WaitFrames>(WaitFramesParams {
                window: parse_window_ref(window)?,
                n: *n,
                timeout: *timeout,
            })?;
            if json {
                print_json(&t)
            } else {
                println!("presented at {} ns", t.0);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_trigger_files_in_both_forms() {
        let one = r#"[{"on": {"type": "input_sent"}, "then": [{"type": "mark", "label": "x"}]}]"#;
        assert_eq!(parse_triggers(one).unwrap().len(), 1);
        let set = format!("{{\"triggers\": {one}}}");
        assert_eq!(parse_triggers(&set).unwrap().len(), 1);
        assert!(parse_triggers("{}").is_err());
    }
}
