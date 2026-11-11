//! The session spec: the short forms of the common settings, applied over a base config.

use std::collections::BTreeMap;

use fakeland::{DbusMode, OutputConfig, SamplingConfig, SessionConfig};
use fakeland_types::{Dur, Hz};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    SpecError, parse_callbacks, parse_composite, parse_output, parse_renderer, parse_timing,
    parse_window_policy,
};

/// Common session settings in their short text forms. Unset fields keep the base config.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SessionSpec {
    /// Outputs as `[NAME:]WIDTHxHEIGHT[@HZ]`, such as `2560x1440@143.912`. Unnamed outputs are
    /// DP-1, DP-2 and so on.
    pub outputs: Vec<String>,
    /// The scale of every output, such as 1.5.
    pub scale: Option<f64>,
    /// The timing of every output: `fixed`, `vrr:MIN-MAX`, `unthrottled`, `manual` or
    /// `manual:HZ`.
    pub timing: Option<String>,
    /// The latch offset before each vblank, such as `1.5ms`.
    pub latch: Option<String>,
    /// Frame callback dispatch: `at-vblank`, `before-vblank:LEAD` or `on-present`.
    pub callbacks: Option<String>,
    /// The renderer: `auto`, `gles` or `pixman`.
    pub renderer: Option<String>,
    /// Children use the host session bus instead of a private bus.
    pub host_dbus: bool,
    /// Advertise the app-mark protocol.
    pub app_marks: bool,
    /// Apps keep their accessibility trees off, and the session runs no accessibility bus.
    pub no_a11y: bool,
    /// `org.a11y.Status` reports a running screen reader.
    pub screen_reader: bool,
    /// Compositing: `lazy` or `every-frame`.
    pub composite: Option<String>,
    /// Window placement: `kiosk`, `floating[:WxH]` or `fullscreen`.
    pub window_policy: Option<String>,
    /// More environment variables for every child.
    pub env: BTreeMap<String, String>,
    /// The Wayland socket name.
    pub socket_name: Option<String>,
    /// The trace ring size in records. 0 keeps every record.
    pub trace_capacity: Option<usize>,
    /// The pointer report rate in Hz.
    pub report_rate: Option<f64>,
    /// Sample CPU, memory and threads of every app and the GPU at this interval, such as
    /// `100ms`.
    pub sample: Option<String>,
    /// Show the outputs in a nested window on the host desktop. The server needs the viewer
    /// feature.
    pub viewer: bool,
    /// The host display of the viewer: a socket name or an absolute path. The default is
    /// `WAYLAND_DISPLAY` of the server.
    pub viewer_display: Option<String>,
}

impl SessionSpec {
    /// Applies the spec to a base config and validates the result.
    pub fn apply(&self, mut config: SessionConfig) -> Result<SessionConfig, SpecError> {
        if !self.outputs.is_empty() {
            let mut outputs = Vec::new();
            for (i, s) in self.outputs.iter().enumerate() {
                let (name, mode) = parse_output(s)?;
                let name = name.unwrap_or_else(|| format!("DP-{}", i + 1));
                let base = config
                    .outputs
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| OutputConfig::new(name.clone()));
                let mut out = base.mode(mode.width, mode.height, mode.refresh);
                out.name = name;
                out.timing = fakeland_timing::TimingConfig::fixed(mode.refresh)
                    .with_latch_offset(out.timing.latch_offset)
                    .with_callbacks(out.timing.callbacks);
                outputs.push(out);
            }
            config.outputs = outputs;
        }
        for out in &mut config.outputs {
            self.apply_output(out)?;
        }
        if let Some(r) = &self.renderer {
            config.renderer = parse_renderer(r)?;
        }
        if self.host_dbus {
            config.dbus = DbusMode::Host;
        }
        if self.no_a11y {
            config.accessibility = false;
        }
        if self.screen_reader {
            config.screen_reader = true;
        }
        if self.app_marks {
            config.app_marks = true;
        }
        if let Some(c) = &self.composite {
            config.composite = parse_composite(c)?;
        }
        if let Some(p) = &self.window_policy {
            config.window_policy = parse_window_policy(p)?;
        }
        config
            .env
            .extend(self.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Some(name) = &self.socket_name {
            config.socket_name = Some(name.clone());
        }
        if let Some(cap) = self.trace_capacity {
            config.trace_capacity = (cap > 0).then_some(cap);
        }
        if let Some(rate) = self.report_rate {
            config.input.pointer.report_rate = Some(Hz(rate));
        }
        if let Some(s) = &self.sample {
            let d: Dur = s.parse().map_err(|_| SpecError::Duration(s.clone()))?;
            config.sampling = Some(SamplingConfig::every(d.0));
        }
        if self.viewer {
            config.viewer = true;
        }
        if let Some(d) = &self.viewer_display {
            config.viewer_display = Some(d.clone());
        }
        config.validate()?;
        Ok(config)
    }

    fn apply_output(&self, out: &mut OutputConfig) -> Result<(), SpecError> {
        if let Some(scale) = self.scale {
            out.scale = scale;
        }
        let mut timing = match &self.timing {
            Some(t) => parse_timing(t, out.mode.refresh)?
                .with_latch_offset(out.timing.latch_offset)
                .with_callbacks(out.timing.callbacks),
            None => out.timing,
        };
        if let Some(l) = &self.latch {
            let d: Dur = l.parse().map_err(|_| SpecError::Duration(l.clone()))?;
            timing = timing.with_latch_offset(d.0);
        }
        if let Some(c) = &self.callbacks {
            timing = timing.with_callbacks(parse_callbacks(c)?);
        }
        out.timing = timing;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland::{Composite, RendererChoice};
    use fakeland_timing::{CallbackPolicy, TimingMode};
    use fakeland_types::Nanos;

    #[test]
    fn applies_every_setting() {
        let spec = SessionSpec {
            outputs: vec!["2560x1440@143.912".into(), "HDMI-A-1:1280x720@60".into()],
            scale: Some(1.5),
            timing: Some("vrr:48-144".into()),
            latch: Some("1.5ms".into()),
            callbacks: Some("before-vblank:3ms".into()),
            renderer: Some("pixman".into()),
            host_dbus: true,
            app_marks: true,
            no_a11y: true,
            screen_reader: true,
            composite: Some("every-frame".into()),
            window_policy: Some("fullscreen".into()),
            env: [("A".to_owned(), "1".to_owned())].into(),
            socket_name: Some("wayland-test".into()),
            trace_capacity: Some(0),
            report_rate: Some(1000.0),
            sample: Some("250ms".into()),
            viewer: true,
            viewer_display: Some("wayland-9".into()),
        };
        let c = spec.apply(SessionConfig::default()).unwrap();
        assert_eq!(c.outputs.len(), 2);
        assert_eq!(c.outputs[0].name, "DP-1");
        assert_eq!(c.outputs[1].name, "HDMI-A-1");
        assert_eq!(c.outputs[0].mode.width, 2560);
        assert_eq!(c.outputs[1].scale, 1.5);
        let t = c.outputs[0].timing;
        assert_eq!(
            t.mode,
            TimingMode::Vrr {
                min: Hz(48.0),
                max: Hz(144.0)
            }
        );
        assert_eq!(t.latch_offset, Nanos::from_micros(1500));
        assert_eq!(
            t.callbacks,
            CallbackPolicy::BeforeVblank {
                lead: Nanos::from_millis(3)
            }
        );
        assert_eq!(c.renderer, RendererChoice::Pixman);
        assert_eq!(c.dbus, DbusMode::Host);
        assert!(c.app_marks);
        assert!(!c.accessibility);
        assert!(c.screen_reader);
        assert_eq!(c.composite, Composite::EveryFrame);
        assert_eq!(c.env.get("A").map(String::as_str), Some("1"));
        assert_eq!(c.trace_capacity, None);
        assert_eq!(c.input.pointer.report_rate, Some(Hz(1000.0)));
        assert_eq!(
            c.sampling,
            Some(SamplingConfig::every(Nanos::from_millis(250)))
        );
        assert!(c.viewer);
        assert_eq!(c.viewer_display.as_deref(), Some("wayland-9"));
    }

    #[test]
    fn keeps_the_base_without_settings() {
        let base = SessionConfig::default();
        assert_eq!(SessionSpec::default().apply(base.clone()).unwrap(), base);
    }

    #[test]
    fn fixed_timing_follows_each_mode() {
        let spec = SessionSpec {
            outputs: vec!["1920x1080@144".into()],
            timing: Some("fixed".into()),
            ..SessionSpec::default()
        };
        let c = spec.apply(SessionConfig::default()).unwrap();
        assert_eq!(
            c.outputs[0].timing.mode,
            TimingMode::Fixed { refresh: Hz(144.0) }
        );
    }

    #[test]
    fn rejects_short_sample_intervals() {
        let spec = SessionSpec {
            sample: Some("1ms".into()),
            ..SessionSpec::default()
        };
        assert!(matches!(
            spec.apply(SessionConfig::default()),
            Err(SpecError::Config(_))
        ));
        let spec = SessionSpec {
            sample: Some("soon".into()),
            ..SessionSpec::default()
        };
        assert!(matches!(
            spec.apply(SessionConfig::default()),
            Err(SpecError::Duration(_))
        ));
    }

    #[test]
    fn rejects_bad_forms() {
        let spec = SessionSpec {
            timing: Some("bogus".into()),
            ..SessionSpec::default()
        };
        assert!(spec.apply(SessionConfig::default()).is_err());
        let spec = SessionSpec {
            scale: Some(100.0),
            ..SessionSpec::default()
        };
        assert!(matches!(
            spec.apply(SessionConfig::default()),
            Err(SpecError::Config(_))
        ));
    }
}
