//! Bench commands of ctl: the bench runs inside the server.

use anyhow::Result;
use fakeland_control::ControlClient;
use fakeland_control::methods::{ListPresets, NoParams, RunBench, TraceFormat, TraceOutput};

use super::args::CtlCommand;
use super::print::emit;
use crate::commands::bench::print_outcome;
use crate::util::absolute;

/// Runs a bench command. Returns `None` for commands of other groups.
pub fn run(c: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    match cmd {
        CtlCommand::Bench {
            select,
            report,
            perfetto,
            baseline,
        } => {
            let mut p = select.bench_params()?;
            p.report = report.as_deref().map(absolute).transpose()?;
            p.baseline = baseline.as_deref().map(absolute).transpose()?;
            if let Some(path) = perfetto {
                p.traces.push(TraceOutput {
                    path: absolute(path)?,
                    format: Some(TraceFormat::Perfetto),
                });
            }
            let outcome = c.blocking_call::<RunBench>(p)?;
            Ok(Some(print_outcome(&outcome, json)?))
        }
        CtlCommand::Presets => {
            let list = c.blocking_call::<ListPresets>(NoParams {})?;
            emit(json, &list, |l| {
                l.iter()
                    .map(|e| format!("{}  {}", e.name, e.description))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
