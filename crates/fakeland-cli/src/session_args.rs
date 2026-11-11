//! The session flags shared by `run`, `bench` and `serve`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use fakeland::SessionConfig;
use fakeland_control::SessionSpec;

/// Flags that configure the session.
#[derive(Debug, Clone, Default, Args)]
pub struct SessionArgs {
    /// An output as `[NAME:]WIDTHxHEIGHT[@HZ]`, such as `2560x1440@143.912`. Repeat for more
    /// outputs.
    #[arg(long = "output", value_name = "MODE", help_heading = "Session")]
    pub outputs: Vec<String>,
    /// The scale of every output, such as 1.5.
    #[arg(long, help_heading = "Session")]
    pub scale: Option<f64>,
    /// Display timing: fixed, vrr:MIN-MAX, unthrottled, manual or manual:HZ.
    #[arg(long, value_name = "MODEL", help_heading = "Session")]
    pub timing: Option<String>,
    /// The latch offset before each vblank, such as 1.5ms.
    #[arg(long, value_name = "DUR", help_heading = "Session")]
    pub latch: Option<String>,
    /// Frame callbacks: at-vblank, before-vblank:LEAD or on-present.
    #[arg(long, value_name = "POLICY", help_heading = "Session")]
    pub callbacks: Option<String>,
    /// The renderer: auto, gles or pixman.
    #[arg(long, help_heading = "Session")]
    pub renderer: Option<String>,
    /// Children use the host session bus instead of a private bus.
    #[arg(long, help_heading = "Session")]
    pub host_dbus: bool,
    /// Advertise the app-mark protocol to clients.
    #[arg(long, help_heading = "Session")]
    pub app_marks: bool,
    /// Keep accessibility trees off and run no accessibility bus.
    #[arg(long, help_heading = "Session")]
    pub no_a11y: bool,
    /// Report a running screen reader on the accessibility bus.
    #[arg(long, help_heading = "Session", conflicts_with = "no_a11y")]
    pub screen_reader: bool,
    /// Compositing: lazy or every-frame.
    #[arg(long, value_name = "MODE", help_heading = "Session")]
    pub composite: Option<String>,
    /// Window placement: kiosk, floating (with an optional :WxH size) or fullscreen.
    #[arg(long, value_name = "POLICY", help_heading = "Session")]
    pub window_policy: Option<String>,
    /// An environment variable for every child as KEY=VALUE. Repeat for more.
    #[arg(long = "env", value_name = "KEY=VALUE", value_parser = parse_env, help_heading = "Session")]
    pub env: Vec<(String, String)>,
    /// The Wayland socket name in XDG_RUNTIME_DIR.
    #[arg(long, value_name = "NAME", help_heading = "Session")]
    pub socket_name: Option<String>,
    /// The trace ring size in records. 0 keeps every record.
    #[arg(long, value_name = "N", help_heading = "Session")]
    pub trace_capacity: Option<usize>,
    /// The pointer report rate in Hz.
    #[arg(long, value_name = "HZ", help_heading = "Session")]
    pub report_rate: Option<f64>,
    /// Sample CPU, memory and threads of every app and the GPU at this interval, such as
    /// 100ms. The samples go into the trace.
    #[arg(long, value_name = "DUR", help_heading = "Session")]
    pub sample: Option<String>,
    /// Show the outputs in a nested window on the host desktop. Needs a build with the viewer
    /// feature.
    #[arg(long, help_heading = "Session")]
    pub viewer: bool,
    /// The host display of the viewer: a socket name or an absolute path. The default is
    /// WAYLAND_DISPLAY.
    #[arg(
        long,
        value_name = "DISPLAY",
        help_heading = "Session",
        requires = "viewer"
    )]
    pub viewer_display: Option<String>,
    /// A session config JSON file. The other session flags apply on top of it.
    #[arg(long, value_name = "FILE", help_heading = "Session")]
    pub config: Option<PathBuf>,
}

fn parse_env(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .ok_or_else(|| format!("{s:?} is not KEY=VALUE"))
}

impl SessionArgs {
    /// Returns the spec of the short forms.
    pub fn spec(&self) -> SessionSpec {
        SessionSpec {
            outputs: self.outputs.clone(),
            scale: self.scale,
            timing: self.timing.clone(),
            latch: self.latch.clone(),
            callbacks: self.callbacks.clone(),
            renderer: self.renderer.clone(),
            host_dbus: self.host_dbus,
            app_marks: self.app_marks,
            no_a11y: self.no_a11y,
            screen_reader: self.screen_reader,
            composite: self.composite.clone(),
            window_policy: self.window_policy.clone(),
            env: self.env.iter().cloned().collect(),
            socket_name: self.socket_name.clone(),
            trace_capacity: self.trace_capacity,
            report_rate: self.report_rate,
            sample: self.sample.clone(),
            viewer: self.viewer,
            viewer_display: self.viewer_display.clone(),
        }
    }

    /// Builds the session config.
    pub fn config(&self) -> Result<SessionConfig> {
        let base = match &self.config {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                serde_json::from_str(&text)
                    .with_context(|| format!("{} is no session config", path.display()))?
            }
            None => SessionConfig::default(),
        };
        Ok(self.spec().apply(base)?)
    }

    /// Starts a session.
    pub fn start(&self) -> Result<fakeland::Session> {
        let config = self.config()?;
        fakeland::Fakeland::start(config).context("cannot start the session")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_pairs() {
        assert_eq!(parse_env("A=1=2").unwrap(), ("A".into(), "1=2".into()));
        assert!(parse_env("=1").is_err());
        assert!(parse_env("A").is_err());
    }

    #[test]
    fn builds_a_config() {
        let args = SessionArgs {
            outputs: vec!["1280x720@144".into()],
            scale: Some(1.25),
            renderer: Some("pixman".into()),
            env: vec![("X".into(), "y".into())],
            ..SessionArgs::default()
        };
        let c = args.config().unwrap();
        assert_eq!(c.outputs[0].mode.width, 1280);
        assert_eq!(c.outputs[0].scale, 1.25);
        assert_eq!(c.renderer, fakeland::RendererChoice::Pixman);
        assert_eq!(c.env.get("X").map(String::as_str), Some("y"));
    }
}
