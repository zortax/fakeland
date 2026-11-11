//! The configuration of a whole session: outputs, input, policies, isolation, socket, child
//! environment and measurement settings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use fakeland_types::Nanos;
use fakeland_types::dur::{Dur, as_dur};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    Composite, ConfigError, DbusMode, Decorations, InputConfig, MIN_SAMPLE_INTERVAL, OutputConfig,
    RendererChoice, SamplingConfig, WindowPolicy,
};

/// Everything a session needs to start. Every field has a default, so `{}` is a valid config.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SessionConfig {
    /// The outputs, in order. The first output is the primary output.
    pub outputs: Vec<OutputConfig>,
    /// The input devices of `seat0`.
    pub input: InputConfig,
    /// The D-Bus session bus of the children.
    pub dbus: DbusMode,
    /// The session advertises the app-mark global.
    pub app_marks: bool,
    /// When outputs composite.
    pub composite: Composite,
    /// The renderer.
    pub renderer: RendererChoice,
    /// The render node to open. `None` selects `/dev/dri/renderD128` or the first render node.
    pub render_node: Option<PathBuf>,
    /// How new windows are placed.
    pub window_policy: WindowPolicy,
    /// Which side draws decorations.
    pub decorations: Decorations,
    /// The socket name in `$XDG_RUNTIME_DIR`. `None` picks a free `wayland-N`.
    pub socket_name: Option<String>,
    /// More environment variables for every child.
    pub env: BTreeMap<String, String>,
    /// A wake later than this after a deadline makes the run invalid.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub lateness_threshold: Nanos,
    /// The timer fires this early and the compositor spins to the deadline. Zero disables the
    /// spin.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub timer_spin: Nanos,
    /// The trace ring keeps this many records. `None` keeps every record.
    pub trace_capacity: Option<usize>,
    /// Each child keeps this many bytes of stdout and of stderr.
    pub log_capacity: usize,
    /// Apps expose their accessibility trees. With the private bus the session runs its own
    /// accessibility bus and registry.
    pub accessibility: bool,
    /// `org.a11y.Status` reports a running screen reader. Some toolkits build a fuller tree for a
    /// screen reader.
    pub screen_reader: bool,
    /// Resource sampling of launched process trees and the GPU. `None` turns sampling off.
    pub sampling: Option<SamplingConfig>,
    /// Show the outputs in a nested window on the host desktop. This needs the `viewer` feature
    /// and a host `WAYLAND_DISPLAY`.
    pub viewer: bool,
    /// The host display of the viewer: a socket name in `$XDG_RUNTIME_DIR` or an absolute path.
    /// `None` takes `WAYLAND_DISPLAY` of this process.
    pub viewer_display: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            outputs: vec![OutputConfig::default()],
            input: InputConfig::default(),
            dbus: DbusMode::Private,
            app_marks: false,
            composite: Composite::Lazy,
            renderer: RendererChoice::Auto,
            render_node: None,
            window_policy: WindowPolicy::Kiosk,
            decorations: Decorations::ClientSide,
            socket_name: None,
            env: BTreeMap::new(),
            lateness_threshold: Nanos::from_millis(2),
            timer_spin: Nanos::ZERO,
            trace_capacity: Some(1 << 20),
            log_capacity: 1 << 20,
            accessibility: true,
            screen_reader: false,
            sampling: None,
            viewer: false,
            viewer_display: None,
        }
    }
}

impl SessionConfig {
    /// Checks every output and setting.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.outputs.is_empty() {
            return Err(ConfigError::NoOutputs);
        }
        let mut names = BTreeSet::new();
        for out in &self.outputs {
            validate_output(out)?;
            if !names.insert(out.name.as_str()) {
                return Err(ConfigError::DuplicateOutput(out.name.clone()));
            }
        }
        if let Some(name) = &self.socket_name
            && (name.is_empty() || name.contains('/'))
        {
            return Err(ConfigError::SocketName(name.clone()));
        }
        if self.lateness_threshold.is_negative() || self.timer_spin.is_negative() {
            return Err(ConfigError::NegativeSpan);
        }
        if let Some(rate) = self.input.pointer.report_rate
            && !(rate.0.is_finite() && rate.0 > 0.0)
        {
            return Err(ConfigError::ReportRate);
        }
        if let Some(s) = &self.sampling
            && s.interval < MIN_SAMPLE_INTERVAL
        {
            return Err(ConfigError::SampleInterval);
        }
        if self.trace_capacity == Some(0) {
            return Err(ConfigError::TraceCapacity);
        }
        if let WindowPolicy::Floating { default_size } = self.window_policy
            && (default_size.w <= 0 || default_size.h <= 0)
        {
            return Err(ConfigError::WindowSize);
        }
        Ok(())
    }

    /// Returns the JSON schema of the config.
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(SessionConfig)
    }
}

/// Checks one output.
pub fn validate_output(out: &OutputConfig) -> Result<(), ConfigError> {
    if out.name.is_empty() {
        return Err(ConfigError::OutputName);
    }
    for mode in std::iter::once(&out.mode).chain(&out.extra_modes) {
        let refresh_ok = mode.refresh.0.is_finite() && mode.refresh.0 > 0.0;
        if mode.width <= 0 || mode.height <= 0 || !refresh_ok {
            return Err(ConfigError::Mode {
                output: out.name.clone(),
            });
        }
    }
    if !(0.25..=8.0).contains(&out.scale) {
        return Err(ConfigError::Scale {
            output: out.name.clone(),
            scale: out.scale,
        });
    }
    out.effective_timing()
        .validate()
        .map_err(|source| ConfigError::Timing {
            output: out.name.clone(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use fakeland_timing::TimingConfig;
    use fakeland_types::{Hz, Size};

    use super::*;

    #[test]
    fn empty_json_is_the_default() {
        let cfg: SessionConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg, SessionConfig::default());
        cfg.validate().unwrap();
    }

    #[test]
    fn round_trips_through_json() {
        let mut cfg = SessionConfig {
            window_policy: WindowPolicy::Floating {
                default_size: Size::new(800, 600),
            },
            lateness_threshold: Nanos::from_micros(500),
            ..SessionConfig::default()
        };
        cfg.env.insert("FOO".into(), "bar".into());
        cfg.outputs.push(OutputConfig::new("DP-2").scale(1.5));
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        assert!(json.contains("\"500us\""));
        let back: SessionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn rejects_bad_configs() {
        let mut cfg = SessionConfig::default();
        cfg.outputs.clear();
        assert!(matches!(cfg.validate(), Err(ConfigError::NoOutputs)));

        let mut cfg = SessionConfig::default();
        cfg.outputs.push(OutputConfig::default());
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::DuplicateOutput(_))
        ));

        let mut cfg = SessionConfig::default();
        cfg.outputs[0].scale = 0.0;
        assert!(matches!(cfg.validate(), Err(ConfigError::Scale { .. })));

        let mut cfg = SessionConfig::default();
        cfg.outputs[0].mode.refresh = Hz(0.0);
        assert!(matches!(cfg.validate(), Err(ConfigError::Mode { .. })));

        let mut cfg = SessionConfig::default();
        cfg.outputs[0].timing = TimingConfig::default().with_latch_offset(Nanos::from_millis(40));
        assert!(matches!(cfg.validate(), Err(ConfigError::Timing { .. })));

        let cfg = SessionConfig {
            socket_name: Some("a/b".into()),
            ..SessionConfig::default()
        };
        assert!(matches!(cfg.validate(), Err(ConfigError::SocketName(_))));
    }

    #[test]
    fn schema_names_the_fields() {
        let schema = serde_json::to_string(&SessionConfig::json_schema()).unwrap();
        for key in ["outputs", "window_policy", "lateness_threshold", "timing"] {
            assert!(schema.contains(key), "{key}");
        }
    }
}
