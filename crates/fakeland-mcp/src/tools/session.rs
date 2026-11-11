//! Session, app, window, output and clipboard tools.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use fakeland::{LaunchSpec, OutputChange, WindowOp};
use fakeland_control::Controller;
use fakeland_control::methods::{
    ClipboardGetParams, ClipboardSetParams as CtlClipboardSet, KillParams, KillSignal,
    LaunchParams as CtlLaunch, LogsParams as CtlLogs, NoParams, RestartParams,
};
use fakeland_control::ops::describe_exit;
use fakeland_control::spec::{parse_output, parse_timing, parse_window_ref};
use fakeland_control::text::outputs_text;
use fakeland_types::{Dur, OutputId, ProcessId};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use super::common::{limit, ms, window_line};
use crate::params::{
    ClipboardSetParams, KillAppParams, LaunchParams, LogsParams, NoArgs, RestartAppParams,
    SessionStartParams, SetOutputParams, WindowParams,
};
use crate::reply::{ToolResult, done, done_json, finish, text};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

fn launch_spec(p: &LaunchParams) -> Result<LaunchSpec> {
    let mut spec = if p.args.is_empty() {
        LaunchSpec::from_argv(p.command.split_whitespace()).context("the command is empty")?
    } else {
        LaunchSpec::new(p.command.trim()).args(p.args.iter().cloned())
    };
    for (k, v) in &p.env {
        spec = spec.env(k, v);
    }
    if let Some(dir) = &p.cwd {
        spec = spec.cwd(PathBuf::from(dir));
    }
    Ok(spec)
}

fn app_line(a: &fakeland_control::methods::AppInfo) -> String {
    let state = a
        .exit
        .map_or_else(|| "running".to_owned(), |e| describe_exit(e.code, e.signal));
    format!("pid {} {} ({state})", a.pid.0, a.command)
}

impl FakelandMcp {
    async fn status_text(&self, c: &Controller) -> Result<String> {
        let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
        let apps = c.apps_list(NoParams {});
        let mut out = format!(
            "Session WAYLAND_DISPLAY={}\nOutputs:\n{}",
            c.session().socket_name(),
            outputs_text(&outputs)
        );
        if !apps.is_empty() {
            out.push_str("Apps:\n");
            for a in &apps {
                out.push_str(&app_line(a));
                out.push('\n');
            }
        }
        out.push_str(&self.windows_line(c).await);
        Ok(out)
    }

    async fn output_changes(
        &self,
        c: &Controller,
        p: &SetOutputParams,
    ) -> Result<Vec<OutputChange>> {
        let mut changes = Vec::new();
        let mut refresh = None;
        if let Some(m) = &p.mode {
            let (_, mode) = parse_output(m)?;
            refresh = Some(mode.refresh);
            changes.push(OutputChange::Mode(mode));
        }
        if let Some(s) = p.scale {
            changes.push(OutputChange::Scale { scale: s });
        }
        if let Some(t) = &p.timing {
            let refresh = match refresh {
                Some(r) => r,
                None => {
                    let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
                    outputs
                        .iter()
                        .find(|o| p.output.is_none_or(|id| o.id == OutputId(id)))
                        .map(|o| o.mode.refresh)
                        .context("no such output")?
                }
            };
            changes.push(OutputChange::Timing(parse_timing(t, refresh)?));
        }
        match p.enabled {
            Some(true) => changes.push(OutputChange::Enable),
            Some(false) => changes.push(OutputChange::Disable),
            None => {}
        }
        if changes.is_empty() {
            bail!("give at least one of mode, scale, timing or enabled");
        }
        Ok(changes)
    }
}

#[tool_router(router = session_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Starts the session with settings.
    #[tool(
        description = "Start the headless session with settings: outputs like \"2560x1440@144\", scale, timing (fixed, vrr:48-144, unthrottled, manual), renderer (auto, gles, pixman), app_marks, window_policy (kiosk, floating, fullscreen), host_dbus, no_a11y (no accessibility tree), screen_reader (apps see a screen reader), sample (\"100ms\" samples CPU, memory and GPU for the resources tool and traces), viewer (shows the session in a window on the host desktop). Other tools start a default session on first use, so call this only for other settings. Pass restart=true to replace a running session."
    )]
    async fn session_start(&self, Parameters(p): Parameters<SessionStartParams>) -> ToolResult {
        finish(
            async {
                let c = self.state.start(&p.spec, p.restart).await?;
                done(self.status_text(&c).await?)
            }
            .await,
        )
    }

    /// Stops the session.
    #[tool(
        description = "Stop the session. Its apps end. The next tool call starts a new default session."
    )]
    async fn session_stop(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        if self.state.stop().await {
            text("The session stopped.")
        } else {
            text("No session was running.")
        }
    }

    /// Launches an app.
    #[tool(
        description = "Launch a GUI app in the session, such as \"alacritty\" or \"gnome-calculator\", and wait for its first window. Returns its pid and window. If the app exits first, the error shows its stderr."
    )]
    async fn launch(&self, Parameters(p): Parameters<LaunchParams>) -> ToolResult {
        finish(async {
            let c = self.ctl().await?;
            let spec = launch_spec(&p)?;
            let wait = (p.wait_window_ms > 0).then(|| Dur(ms(p.wait_window_ms)));
            let info = bounded(
                limit(p.wait_window_ms),
                c.launch(CtlLaunch {
                    spec,
                    wait_window: wait,
                }),
            )
            .await??;
            let window = match &info.window {
                Some(w) => format!("\nFirst window: {}", window_line(w)),
                None if wait.is_some() => "\nNo window appeared within the wait; it may still come. Use wait_for with until=window.".to_owned(),
                None => String::new(),
            };
            done(format!("Launched {}{window}", app_line(&info)))
        }
        .await)
    }

    /// Lists windows, outputs and apps.
    #[tool(
        description = "List the windows (id, app_id, title, logical geometry, focus), the outputs and the launched apps."
    )]
    async fn list_windows(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let windows = bounded(CALL_SLACK, c.session().windows()).await??;
                let text = self.status_text(&c).await?;
                done_json(text, &windows)
            }
            .await,
        )
    }

    /// Focuses a window.
    #[tool(
        description = "Give a window keyboard focus and raise it. window: focused, any, an id such as 3, app:APP_ID or title:REGEX."
    )]
    async fn focus_window(&self, Parameters(p): Parameters<WindowParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let matcher = parse_window_ref(&p.window)?;
                bounded(
                    CALL_SLACK,
                    c.session().window_op(matcher.clone(), WindowOp::Focus),
                )
                .await??;
                let w = c.resolve_window(matcher).await?;
                done(format!("Focused {}", window_line(&w)))
            }
            .await,
        )
    }

    /// Returns app output.
    #[tool(
        description = "Show the last lines of stdout and stderr of a launched app, and whether it still runs."
    )]
    async fn logs(&self, Parameters(p): Parameters<LogsParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let logs = c.logs(CtlLogs {
                    pid: p.pid.map(ProcessId),
                    lines: p.lines,
                })?;
                done(format!(
                    "{}\n--- stdout\n{}\n--- stderr\n{}",
                    app_line(&logs.app),
                    logs.stdout,
                    logs.stderr
                ))
            }
            .await,
        )
    }

    /// Stops an app.
    #[tool(
        description = "Stop a launched app with SIGTERM (or SIGKILL with force) and wait for its exit."
    )]
    async fn kill_app(&self, Parameters(p): Parameters<KillAppParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let info = bounded(
                    limit(5000),
                    c.kill(KillParams {
                        pid: p.pid.map(ProcessId),
                        signal: if p.force {
                            KillSignal::Kill
                        } else {
                            KillSignal::Term
                        },
                        wait: Some(Dur::from_secs(3)),
                    }),
                )
                .await??;
                done(format!("Stopped {}", app_line(&info)))
            }
            .await,
        )
    }

    /// Restarts an app.
    #[tool(
        description = "Stop a launched app and start it again with the same command, then wait for its first window."
    )]
    async fn restart_app(&self, Parameters(p): Parameters<RestartAppParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let wait = (p.wait_window_ms > 0).then(|| Dur(ms(p.wait_window_ms)));
                let info = bounded(
                    limit(p.wait_window_ms + 5000),
                    c.restart(RestartParams {
                        pid: p.pid.map(ProcessId),
                        wait_window: wait,
                    }),
                )
                .await??;
                let window = info.window.as_ref().map_or_else(String::new, |w| {
                    format!("\nFirst window: {}", window_line(w))
                });
                done(format!("Restarted as {}{window}", app_line(&info)))
            }
            .await,
        )
    }

    /// Changes an output.
    #[tool(
        description = "Change an output: mode like \"2560x1440@144\", scale like 1.5, timing (fixed, vrr:48-144, unthrottled, manual), or enabled. Windows follow the new size."
    )]
    async fn set_output(&self, Parameters(p): Parameters<SetOutputParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let output = p.output.map(OutputId);
                for change in self.output_changes(&c, &p).await? {
                    bounded(CALL_SLACK, c.session().set_output(output, change)).await??;
                }
                let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
                done(format!("Outputs:\n{}", outputs_text(&outputs)))
            }
            .await,
        )
    }

    /// Reads the clipboard.
    #[tool(description = "Read the clipboard text of the session.")]
    async fn clipboard_get(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let t =
                    bounded(limit(2000), c.clipboard_get(ClipboardGetParams::default())).await??;
                done(t)
            }
            .await,
        )
    }

    /// Sets the clipboard.
    #[tool(
        description = "Set the clipboard text of the session, for example before pasting with key ctrl+v."
    )]
    async fn clipboard_set(&self, Parameters(p): Parameters<ClipboardSetParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let n = p.text.chars().count();
                bounded(
                    CALL_SLACK,
                    c.clipboard_set(CtlClipboardSet {
                        text: p.text,
                        mime: fakeland_control::methods::defaults::text_mime(),
                    }),
                )
                .await??;
                done(format!("The clipboard holds {n} characters."))
            }
            .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_command_lines() {
        let p = LaunchParams {
            command: "alacritty -e cat".into(),
            args: Vec::new(),
            env: [("A".to_owned(), "1".to_owned())].into(),
            cwd: Some("/tmp".into()),
            wait_window_ms: 0,
        };
        let spec = launch_spec(&p).unwrap();
        assert_eq!(spec.program, "alacritty");
        assert_eq!(spec.args, ["-e", "cat"]);
        assert_eq!(spec.env, [("A".to_owned(), "1".to_owned())]);
        let p = LaunchParams {
            command: "/opt/my app".into(),
            args: vec!["--x".into()],
            env: Default::default(),
            cwd: None,
            wait_window_ms: 0,
        };
        assert_eq!(launch_spec(&p).unwrap().program, "/opt/my app");
    }
}
