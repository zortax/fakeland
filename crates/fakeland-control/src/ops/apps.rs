//! Launched apps: the registry of handles and the launch, signal, restart and log operations.

use std::collections::BTreeMap;

use fakeland::{App, LaunchSpec, SessionError};
use fakeland_types::{Nanos, ProcessId};

use super::timer::with_timeout;
use super::{Controller, OpError};
use crate::methods::{
    AppInfo, KillParams, KillSignal, LaunchParams, Logs, LogsParams, NoParams, RestartParams,
};

/// The apps a controller launched, by pid, with their launch specs.
#[derive(Debug, Default)]
pub struct AppRegistry {
    apps: BTreeMap<u32, (App, LaunchSpec)>,
    newest: Option<u32>,
}

impl AppRegistry {
    fn insert(&mut self, app: App, spec: LaunchSpec) {
        let pid = app.pid().0;
        self.apps.insert(pid, (app, spec));
        self.newest = Some(pid);
    }

    fn get(&self, pid: Option<ProcessId>) -> Result<(App, LaunchSpec), OpError> {
        let key = match pid {
            Some(p) => p.0,
            None => self
                .newest
                .ok_or_else(|| OpError::NotFound("no app was launched".into()))?,
        };
        self.apps
            .get(&key)
            .cloned()
            .ok_or_else(|| OpError::NotFound(format!("no app with pid {key}")))
    }
}

/// Returns the public view of an app.
pub fn app_info(app: &App) -> AppInfo {
    AppInfo {
        pid: app.pid(),
        command: app.command().to_owned(),
        exit: app.try_wait(),
        window: None,
    }
}

/// Returns the last `n` lines of a text.
pub fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

impl Controller {
    /// Launches an app and registers it. With `wait_window`, the call waits for the first mapped
    /// window. An app that exits first fails the call with the tail of its stderr.
    pub async fn launch(&self, p: LaunchParams) -> Result<AppInfo, OpError> {
        if p.spec.program.trim().is_empty() {
            return Err(OpError::InvalidParams("the program is empty".into()));
        }
        let app = self.session().launch(p.spec.clone()).await?;
        self.apps().insert(app.clone(), p.spec);
        self.finish_launch(app, p.wait_window.map(|d| d.0)).await
    }

    async fn finish_launch(&self, app: App, wait: Option<Nanos>) -> Result<AppInfo, OpError> {
        let mut info = app_info(&app);
        let Some(timeout) = wait else {
            return Ok(info);
        };
        match app.wait_window(timeout).await {
            Ok(w) => info.window = Some(w),
            Err(SessionError::AppExited) => {
                let exit = app.wait().await;
                return Err(OpError::Failed(format!(
                    "{} exited ({}) before it mapped a window; stderr:\n{}",
                    app.command(),
                    describe_exit(exit.code, exit.signal),
                    tail_lines(&app.stderr(), 20)
                )));
            }
            Err(SessionError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
        info.exit = app.try_wait();
        Ok(info)
    }

    /// Returns every launched app.
    pub fn apps_list(&self, _: NoParams) -> Vec<AppInfo> {
        self.apps()
            .apps
            .values()
            .map(|(a, _)| app_info(a))
            .collect()
    }

    /// Returns the handle of a launched app. `None` selects the newest app.
    pub fn app(&self, pid: Option<ProcessId>) -> Result<App, OpError> {
        Ok(self.apps().get(pid)?.0)
    }

    /// Sends a signal to an app, optionally waiting for its exit.
    pub async fn kill(&self, p: KillParams) -> Result<AppInfo, OpError> {
        let app = self.app(p.pid)?;
        stop_app(&app, p.signal, p.wait.map(|d| d.0)).await?;
        Ok(app_info(&app))
    }

    /// Ends an app and launches its command again.
    pub async fn restart(&self, p: RestartParams) -> Result<AppInfo, OpError> {
        let (app, spec) = self.apps().get(p.pid)?;
        stop_app(&app, KillSignal::Term, Some(Nanos::from_secs(3))).await?;
        let next = self.session().launch(spec.clone()).await?;
        self.apps().insert(next.clone(), spec);
        self.finish_launch(next, p.wait_window.map(|d| d.0)).await
    }

    /// Returns the last lines of stdout and stderr of an app.
    pub fn logs(&self, p: LogsParams) -> Result<Logs, OpError> {
        let app = self.app(p.pid)?;
        Ok(Logs {
            app: app_info(&app),
            stdout: tail_lines(&app.stdout(), p.lines),
            stderr: tail_lines(&app.stderr(), p.lines),
        })
    }
}

async fn stop_app(app: &App, signal: KillSignal, wait: Option<Nanos>) -> Result<(), OpError> {
    let send = |s: KillSignal| match s {
        KillSignal::Term => app.terminate(),
        KillSignal::Kill => app.kill(),
    };
    send(signal).map_err(|e| OpError::Failed(format!("signal failed: {e}")))?;
    let Some(limit) = wait else {
        return Ok(());
    };
    if with_timeout(Box::pin(app.wait()), limit).await.is_some() {
        return Ok(());
    }
    if signal == KillSignal::Kill {
        return Err(OpError::Timeout);
    }
    send(KillSignal::Kill).map_err(|e| OpError::Failed(format!("signal failed: {e}")))?;
    with_timeout(Box::pin(app.wait()), Nanos::from_secs(2))
        .await
        .map(|_| ())
        .ok_or(OpError::Timeout)
}

/// Describes an exit status.
pub fn describe_exit(code: Option<i32>, signal: Option<i32>) -> String {
    match (code, signal) {
        (Some(c), _) => format!("exit code {c}"),
        (None, Some(s)) => format!("signal {s}"),
        (None, None) => "unknown status".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tails_lines() {
        assert_eq!(tail_lines("a\nb\nc\n", 2), "b\nc");
        assert_eq!(tail_lines("a", 5), "a");
        assert_eq!(tail_lines("", 3), "");
    }

    #[test]
    fn describes_exits() {
        assert_eq!(describe_exit(Some(1), None), "exit code 1");
        assert_eq!(describe_exit(None, Some(9)), "signal 9");
    }
}
