//! Session start: `Fakeland::start` validates the config, spawns the compositor thread and returns
//! once the socket listens. `FakelandBuilder` assembles a config in code.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fakeland_timing::TimingConfig;
use fakeland_types::{Logical, Nanos, Size};

use super::{Session, SessionInner, StartError};
use crate::config::{
    Composite, DbusMode, Decorations, InputConfig, OutputConfig, RendererChoice, SamplingConfig,
    SessionConfig, WindowPolicy,
};

/// The entry point of the library.
#[derive(Debug, Clone, Copy)]
pub struct Fakeland;

impl Fakeland {
    /// Returns a builder with the default config and no outputs yet.
    pub fn builder() -> FakelandBuilder {
        FakelandBuilder::default()
    }

    /// Starts a session.
    pub fn start(config: SessionConfig) -> Result<Session, StartError> {
        config.validate()?;
        let clock: fakeland_types::SharedClock = Arc::new(fakeland_types::MonotonicClock);
        let started = crate::server::spawn(config.clone(), clock.clone())?;
        let inner = SessionInner {
            tx: started.tx,
            thread_id: started.thread.thread().id(),
            thread: Mutex::new(Some(started.thread)),
            socket_name: started.socket_name,
            clock,
            environment: started.environment,
            dbus_address: started.dbus_address,
            config,
        };
        Ok(Session::from_inner(Arc::new(inner)))
    }
}

/// Builds a session config.
#[derive(Debug, Clone, Default)]
pub struct FakelandBuilder {
    config: SessionConfig,
    outputs: Vec<OutputConfig>,
}

impl FakelandBuilder {
    /// Adds an output. Without outputs the session gets the default output.
    pub fn output(mut self, output: OutputConfig) -> Self {
        self.outputs.push(output);
        self
    }

    /// Sets the timing of every output added so far, or of the default output.
    pub fn timing(mut self, timing: TimingConfig) -> Self {
        if self.outputs.is_empty() {
            self.outputs.push(OutputConfig::default());
        }
        for o in &mut self.outputs {
            o.timing = timing;
        }
        self
    }

    /// Sets the input devices.
    pub fn input(mut self, input: InputConfig) -> Self {
        self.config.input = input;
        self
    }

    /// Selects the D-Bus bus of the children.
    pub fn dbus(mut self, mode: DbusMode) -> Self {
        self.config.dbus = mode;
        self
    }

    /// Enables the app-mark global.
    pub fn app_marks(mut self, on: bool) -> Self {
        self.config.app_marks = on;
        self
    }

    /// Turns accessibility trees on or off.
    pub fn accessibility(mut self, on: bool) -> Self {
        self.config.accessibility = on;
        self
    }

    /// Makes `org.a11y.Status` report a running screen reader.
    pub fn screen_reader(mut self, on: bool) -> Self {
        self.config.screen_reader = on;
        self
    }

    /// Selects when outputs composite.
    pub fn composite(mut self, composite: Composite) -> Self {
        self.config.composite = composite;
        self
    }

    /// Selects the renderer.
    pub fn renderer(mut self, renderer: RendererChoice) -> Self {
        self.config.renderer = renderer;
        self
    }

    /// Sets the window policy.
    pub fn window_policy(mut self, policy: WindowPolicy) -> Self {
        self.config.window_policy = policy;
        self
    }

    /// Floats new windows at a default size.
    pub fn floating(self, size: Size<Logical>) -> Self {
        self.window_policy(WindowPolicy::Floating { default_size: size })
    }

    /// Selects which side draws decorations.
    pub fn decorations(mut self, decorations: Decorations) -> Self {
        self.config.decorations = decorations;
        self
    }

    /// Sets the socket name.
    pub fn socket_name(mut self, name: impl Into<String>) -> Self {
        self.config.socket_name = Some(name.into());
        self
    }

    /// Adds an environment variable for every child.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.env.insert(key.into(), value.into());
        self
    }

    /// Replaces the extra environment of every child.
    pub fn envs(mut self, env: BTreeMap<String, String>) -> Self {
        self.config.env = env;
        self
    }

    /// Sets the lateness threshold.
    pub fn lateness_threshold(mut self, threshold: Nanos) -> Self {
        self.config.lateness_threshold = threshold;
        self
    }

    /// Sets the timer spin.
    pub fn timer_spin(mut self, spin: Nanos) -> Self {
        self.config.timer_spin = spin;
        self
    }

    /// Sets the trace ring capacity. `None` keeps every record.
    pub fn trace_capacity(mut self, capacity: Option<usize>) -> Self {
        self.config.trace_capacity = capacity;
        self
    }

    /// Turns on resource sampling.
    pub fn sampling(mut self, sampling: SamplingConfig) -> Self {
        self.config.sampling = Some(sampling);
        self
    }

    /// Shows the outputs in a nested window on the host desktop. This needs the `viewer`
    /// feature.
    pub fn viewer(mut self, on: bool) -> Self {
        self.config.viewer = on;
        self
    }

    /// Returns the assembled config.
    pub fn build(self) -> SessionConfig {
        let mut config = self.config;
        if !self.outputs.is_empty() {
            config.outputs = self.outputs;
        }
        config
    }

    /// Starts the session.
    pub fn start(self) -> Result<Session, StartError> {
        Fakeland::start(self.build())
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Hz;

    use super::*;

    #[test]
    fn builder_collects_outputs_and_settings() {
        let config = Fakeland::builder()
            .output(
                OutputConfig::new("DP-1")
                    .mode(2560, 1440, Hz(144.0))
                    .scale(1.5),
            )
            .output(OutputConfig::new("DP-2"))
            .timing(TimingConfig::default().with_latch_offset(Nanos::from_micros(1500)))
            .app_marks(true)
            .env("A", "1")
            .build();
        assert_eq!(config.outputs.len(), 2);
        assert_eq!(config.outputs[0].scale, 1.5);
        assert_eq!(
            config.outputs[1].timing.latch_offset,
            Nanos::from_micros(1500)
        );
        assert!(config.app_marks);
        assert_eq!(config.env["A"], "1");
        config.validate().unwrap();
    }

    fn send_sync<T: Send + Sync>() {}

    #[test]
    fn handles_are_send_and_sync() {
        send_sync::<Session>();
        send_sync::<crate::App>();
        send_sync::<crate::Subscription>();
        send_sync::<crate::Screenshot>();
    }

    #[test]
    fn builder_without_outputs_keeps_the_default_output() {
        let config = Fakeland::builder().build();
        assert_eq!(config.outputs, vec![OutputConfig::default()]);
    }
}
