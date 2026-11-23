//! Output commands of ctl: listing, runtime changes, hotplug and replug.

use anyhow::{Context, Result, bail};
use clap::Args;
use fakeland::{OutputChange, OutputConfig};
use fakeland_control::ControlClient;
use fakeland_control::methods::*;
use fakeland_control::spec::{new_output, parse_output, parse_timing};
use fakeland_control::text::{outputs_text, unplugged_text};
use fakeland_types::OutputId;

use super::args::CtlCommand;
use super::print::emit;

/// Flags of `fakeland ctl set-output`.
#[derive(Debug, Args)]
pub struct SetOutputArgs {
    /// The output id. The default is the primary output.
    #[arg(long, value_name = "ID")]
    output: Option<u32>,
    /// A new mode as WIDTHxHEIGHT, with an optional @HZ refresh.
    #[arg(long, value_name = "MODE")]
    mode: Option<String>,
    /// A new scale.
    #[arg(long)]
    scale: Option<f64>,
    /// A new timing model.
    #[arg(long, value_name = "MODEL")]
    timing: Option<String>,
    /// Power the output on.
    #[arg(long)]
    enable: bool,
    /// Power the output off.
    #[arg(long, conflicts_with = "enable")]
    disable: bool,
}

/// Flags of `fakeland ctl add-output`.
#[derive(Debug, Args)]
pub struct AddOutputArgs {
    /// The output as [NAME:]WIDTHxHEIGHT[@HZ]. The default name is the first free DP-N.
    spec: String,
    /// The scale.
    #[arg(long)]
    scale: Option<f64>,
    /// The position in the global space as X,Y. The default is right of the rightmost output.
    #[arg(long, value_name = "X,Y")]
    position: Option<String>,
}

/// Builds the config of a new output. `taken` holds the names in use.
fn add_config(a: &AddOutputArgs, taken: &[String]) -> Result<OutputConfig> {
    let mut config = new_output(&a.spec, taken)?;
    if let Some(s) = a.scale {
        config = config.scale(s);
    }
    if let Some(p) = &a.position {
        let (x, y) = p
            .split_once(',')
            .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
            .with_context(|| format!("{p:?} is not X,Y"))?;
        config = config.position(x, y);
    }
    Ok(config)
}

async fn output_changes(c: &ControlClient, a: &SetOutputArgs) -> Result<Vec<OutputChange>> {
    let output = a.output.map(OutputId);
    let mut changes = Vec::new();
    let mut refresh = None;
    if let Some(m) = &a.mode {
        let (_, mode) = parse_output(m)?;
        refresh = Some(mode.refresh);
        changes.push(OutputChange::Mode(mode));
    }
    if let Some(s) = a.scale {
        changes.push(OutputChange::Scale { scale: s });
    }
    if let Some(t) = &a.timing {
        let refresh = match refresh {
            Some(r) => r,
            None => {
                let outputs = c.call::<Outputs>(NoParams {}).await?;
                outputs
                    .iter()
                    .find(|o| output.is_none_or(|id| o.id == id))
                    .map(|o| o.mode.refresh)
                    .context("no such output")?
            }
        };
        changes.push(OutputChange::Timing(parse_timing(t, refresh)?));
    }
    if a.enable {
        changes.push(OutputChange::Enable);
    }
    if a.disable {
        changes.push(OutputChange::Disable);
    }
    if changes.is_empty() {
        bail!("name a change: --mode, --scale, --timing, --enable or --disable");
    }
    Ok(changes)
}

/// Runs an output command. Returns `None` for commands of other groups.
pub fn run(c: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    match cmd {
        CtlCommand::Outputs => {
            let outputs = c.blocking_call::<Outputs>(NoParams {})?;
            if json {
                emit(json, &outputs, |_| String::new())?;
            } else {
                let unplugged = c.blocking_call::<UnpluggedOutputs>(NoParams {})?;
                print!("{}{}", outputs_text(&outputs), unplugged_text(&unplugged));
            }
        }
        CtlCommand::SetOutput(a) => {
            let output = a.output.map(OutputId);
            let changes = fakeland::block_on(output_changes(c, a))?;
            let mut last = None;
            for change in changes {
                last = Some(c.blocking_call::<SetOutput>(SetOutputParams { output, change })?);
            }
            if let Some(state) = last {
                emit(json, &state, |s| outputs_text(std::slice::from_ref(s)))?;
            }
        }
        CtlCommand::AddOutput(a) => {
            let taken: Vec<String> = c
                .blocking_call::<Outputs>(NoParams {})?
                .into_iter()
                .map(|o| o.name)
                .collect();
            let config = add_config(a, &taken)?;
            let state = c.blocking_call::<AddOutput>(AddOutputParams { config })?;
            emit(json, &state, |s| outputs_text(std::slice::from_ref(s)))?;
        }
        CtlCommand::RemoveOutput { output } => {
            c.blocking_call::<RemoveOutput>(RemoveOutputParams {
                output: OutputId(*output),
            })?;
        }
        CtlCommand::ReplugOutput { name } => {
            let state =
                c.blocking_call::<ReplugOutput>(ReplugOutputParams { name: name.clone() })?;
            emit(json, &state, |s| outputs_text(std::slice::from_ref(s)))?;
        }
        _ => return Ok(None),
    }
    Ok(Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(spec: &str, scale: Option<f64>, position: Option<&str>) -> AddOutputArgs {
        AddOutputArgs {
            spec: spec.into(),
            scale,
            position: position.map(Into::into),
        }
    }

    #[test]
    fn builds_new_output_configs() {
        let taken = ["DP-1".to_owned(), "DP-2".to_owned()];
        let c = add_config(&args("1280x1024@75", None, None), &taken).unwrap();
        assert_eq!(c.name, "DP-3");
        assert_eq!((c.mode.width, c.mode.height), (1280, 1024));
        assert_eq!(c.position, None);
        let c = add_config(
            &args("HDMI-A-1:800x600", Some(2.0), Some("0, 1080")),
            &taken,
        )
        .unwrap();
        assert_eq!(c.name, "HDMI-A-1");
        assert_eq!(c.scale, 2.0);
        let p = c.position.unwrap();
        assert_eq!((p.x, p.y), (0, 1080));
        assert!(add_config(&args("800x600", None, Some("x")), &taken).is_err());
        assert!(add_config(&args("big", None, None), &taken).is_err());
    }
}
