//! Session, app, output, window and clipboard commands of ctl.

use anyhow::{Context, Result, anyhow, bail};
use fakeland::{LaunchSpec, OutputChange, WindowOp};
use fakeland_control::ControlClient;
use fakeland_control::methods::*;
use fakeland_control::spec::{parse_output, parse_size, parse_timing, parse_window_ref};
use fakeland_control::text::{outputs_text, windows_text};
use fakeland_scenario::{CaptureTarget, Roi};
use fakeland_types::{Logical, OutputId, ProcessId, Rect, Size};

use super::args::{ClipboardCommand, CtlCommand};
use super::print::emit;
use crate::util::absolute;

/// Parses `X,Y,W,H`.
pub fn parse_rect(s: &str) -> Result<Rect<Logical>> {
    let parts: Vec<i32> = s
        .split(',')
        .map(|p| p.trim().parse::<i32>())
        .collect::<Result<_, _>>()
        .map_err(|_| anyhow!("{s:?} is not X,Y,W,H"))?;
    match parts[..] {
        [x, y, w, h] if w > 0 && h > 0 => Ok(Rect::new(x, y, w, h)),
        _ => bail!("{s:?} is not X,Y,W,H with a positive size"),
    }
}

fn app_text(a: &AppInfo) -> String {
    let state = match a.exit {
        Some(e) => fakeland_control::ops::describe_exit(e.code, e.signal),
        None => "running".to_owned(),
    };
    let window = a
        .window
        .as_ref()
        .map_or_else(String::new, |w| format!(" window {} {:?}", w.id, w.app_id));
    format!("{} {} ({state}){window}", a.pid.0, a.command)
}

fn window_op(op: &str, arg: Option<&str>) -> Result<WindowOp> {
    let need = || arg.ok_or_else(|| anyhow!("{op} needs an argument"));
    Ok(match op {
        "focus" => WindowOp::Focus,
        "maximize" => WindowOp::Maximize,
        "fullscreen" => WindowOp::Fullscreen,
        "close" => WindowOp::Close,
        "suspend" => WindowOp::Suspend { suspended: true },
        "resume" => WindowOp::Suspend { suspended: false },
        "resize" => {
            let (w, h) = parse_size(need()?)?;
            WindowOp::Resize {
                size: Size::new(w, h),
            }
        }
        "float" => WindowOp::Floating {
            rect: parse_rect(need()?)?,
        },
        "output" => WindowOp::MoveToOutput {
            output: OutputId(need()?.parse().context("the output id is a number")?),
        },
        other => bail!("unknown window operation {other:?}"),
    })
}

fn shot_target(
    window: Option<&str>,
    output: Option<u32>,
    region: Option<&str>,
) -> Result<CaptureTarget> {
    let output = output.map(OutputId);
    let rect = region.map(parse_rect).transpose()?;
    Ok(match (window, rect) {
        (Some(w), None) => CaptureTarget::Window {
            window: parse_window_ref(w)?,
        },
        (Some(w), Some(r)) => CaptureTarget::Roi {
            roi: Roi::window_rect(parse_window_ref(w)?, r),
        },
        (None, Some(r)) => CaptureTarget::Roi {
            roi: Roi::Output {
                output,
                rect: Some(r),
            },
        },
        (None, None) => CaptureTarget::Output { output },
    })
}

async fn output_changes(
    c: &ControlClient,
    output: Option<OutputId>,
    mode: Option<&str>,
    scale: Option<f64>,
    timing: Option<&str>,
    power: Option<bool>,
) -> Result<Vec<OutputChange>> {
    let mut changes = Vec::new();
    let mut refresh = None;
    if let Some(m) = mode {
        let (_, mode) = parse_output(m)?;
        refresh = Some(mode.refresh);
        changes.push(OutputChange::Mode(mode));
    }
    if let Some(s) = scale {
        changes.push(OutputChange::Scale { scale: s });
    }
    if let Some(t) = timing {
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
    match power {
        Some(true) => changes.push(OutputChange::Enable),
        Some(false) => changes.push(OutputChange::Disable),
        None => {}
    }
    if changes.is_empty() {
        bail!("name a change: --mode, --scale, --timing, --enable or --disable");
    }
    Ok(changes)
}

/// Runs a session command. Returns `None` for commands of other groups.
pub fn run(c: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    match cmd {
        CtlCommand::Info => emit(json, &c.blocking_call::<Info>(NoParams {})?, |i| {
            format!(
                "WAYLAND_DISPLAY={}\nsocket={}\nbus={}\noutputs={}\nrenderer={:?}\ngpu={} driver={}",
                i.wayland_display,
                c.path().display(),
                i.dbus_address.as_deref().unwrap_or("host"),
                i.config.outputs.len(),
                i.config.renderer,
                i.environment.gpu,
                i.environment.driver
            )
        })?,
        CtlCommand::Windows => emit(json, &c.blocking_call::<Windows>(NoParams {})?, |w| {
            windows_text(w)
        })?,
        CtlCommand::Outputs => emit(json, &c.blocking_call::<Outputs>(NoParams {})?, |o| {
            outputs_text(o)
        })?,
        CtlCommand::Apps => emit(json, &c.blocking_call::<Apps>(NoParams {})?, |apps| {
            apps.iter().map(app_text).collect::<Vec<_>>().join("\n")
        })?,
        CtlCommand::Launch { wait, env, command } => {
            let mut spec = LaunchSpec::from_argv(command.clone()).context("no command")?;
            for kv in env {
                let (k, v) = kv
                    .split_once('=')
                    .with_context(|| format!("{kv:?} is not KEY=VALUE"))?;
                spec = spec.env(k, v);
            }
            let spec = match std::env::current_dir() {
                Ok(dir) => spec.cwd(dir),
                Err(_) => spec,
            };
            let info = c.blocking_call::<Launch>(LaunchParams {
                spec,
                wait_window: *wait,
            })?;
            emit(json, &info, app_text)?;
        }
        CtlCommand::Kill { pid, force } => {
            let info = c.blocking_call::<Kill>(KillParams {
                pid: pid.map(ProcessId),
                signal: if *force {
                    KillSignal::Kill
                } else {
                    KillSignal::Term
                },
                wait: Some(fakeland_types::Dur::from_secs(3)),
            })?;
            emit(json, &info, app_text)?;
        }
        CtlCommand::Logs { pid, lines } => {
            let logs = c.blocking_call::<AppLogs>(LogsParams {
                pid: pid.map(ProcessId),
                lines: *lines,
            })?;
            emit(json, &logs, |l| {
                format!(
                    "== {}\n-- stdout\n{}\n-- stderr\n{}",
                    app_text(&l.app),
                    l.stdout,
                    l.stderr
                )
            })?;
        }
        CtlCommand::Shot {
            file,
            window,
            output,
            region,
            max_dim,
            cursor,
        } => {
            let path = absolute(file)?;
            let info = c.blocking_call::<Screenshot>(ScreenshotParams {
                target: shot_target(window.as_deref(), *output, region.as_deref())?,
                include_cursor: *cursor,
                max_dim: *max_dim,
                path: Some(path.clone()),
                inline: false,
            })?;
            emit(json, &info, |s| {
                format!(
                    "{}x{} -> {} (region {}x{}+{}+{}, {:.4} logical px per pixel)",
                    s.width,
                    s.height,
                    path.display(),
                    s.region.w,
                    s.region.h,
                    s.region.x,
                    s.region.y,
                    s.scale
                )
            })?;
        }
        CtlCommand::Mark { label } => {
            c.blocking_call::<Mark>(MarkParams {
                label: label.clone(),
            })?;
        }
        CtlCommand::Clipboard(ClipboardCommand::Get) => {
            let text = c.blocking_call::<ClipboardGet>(ClipboardGetParams::default())?;
            emit(json, &text, |t| t.clone())?;
        }
        CtlCommand::Clipboard(ClipboardCommand::Set { text }) => {
            c.blocking_call::<ClipboardSet>(ClipboardSetParams {
                text: text.clone(),
                mime: defaults::text_mime(),
            })?;
        }
        CtlCommand::Step { n, output } => {
            c.blocking_call::<Step>(StepParams {
                output: output.map(OutputId),
                n: *n,
            })?;
        }
        CtlCommand::SetOutput {
            output,
            mode,
            scale,
            timing,
            enable,
            disable,
        } => {
            let output = output.map(OutputId);
            let power = match (enable, disable) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            let changes = fakeland::block_on(output_changes(
                c,
                output,
                mode.as_deref(),
                *scale,
                timing.as_deref(),
                power,
            ))?;
            let mut last = None;
            for change in changes {
                last = Some(c.blocking_call::<SetOutput>(SetOutputParams { output, change })?);
            }
            if let Some(state) = last {
                emit(json, &state, |s| outputs_text(std::slice::from_ref(s)))?;
            }
        }
        CtlCommand::Window { op, arg, window } => {
            c.blocking_call::<WindowOperation>(WindowOpParams {
                window: parse_window_ref(window)?,
                op: window_op(op, arg.as_deref())?,
            })?;
        }
        CtlCommand::Raw { method, params } => {
            let params = match params {
                Some(p) => serde_json::from_str(p).context("the params are no JSON")?,
                None => serde_json::Value::Null,
            };
            let value = c.blocking_call_raw(method, params)?;
            crate::util::print_json(&value)?;
        }
        CtlCommand::Stop => {
            c.blocking_call::<Shutdown>(NoParams {})?;
            eprintln!("fakeland: the session shut down");
        }
        _ => return Ok(None),
    }
    Ok(Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rects_and_ops() {
        assert_eq!(parse_rect("1,2,30,40").unwrap(), Rect::new(1, 2, 30, 40));
        assert!(parse_rect("1,2,0,40").is_err());
        assert!(parse_rect("1,2").is_err());
        assert_eq!(
            window_op("resize", Some("640x480")).unwrap(),
            WindowOp::Resize {
                size: Size::new(640, 480)
            }
        );
        assert!(window_op("resize", None).is_err());
        assert!(window_op("spin", None).is_err());
    }

    #[test]
    fn builds_shot_targets() {
        assert_eq!(
            shot_target(None, None, None).unwrap(),
            CaptureTarget::Output { output: None }
        );
        assert!(matches!(
            shot_target(Some("focused"), None, Some("0,0,10,10")).unwrap(),
            CaptureTarget::Roi { .. }
        ));
    }
}
