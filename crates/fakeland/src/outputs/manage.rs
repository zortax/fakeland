//! Output management on the compositor thread: hotplug and replug, runtime changes, manual steps
//! and the relayout of the windows, layer surfaces and lock surfaces of a changed output.

use fakeland_timing::{ScheduleError, TimingMode};
use fakeland_trace::Event;
use fakeland_types::OutputId;
use thiserror::Error;

use super::{FakeOutput, OutputChange};
use crate::config::{ConfigError, OutputConfig, validate_output};
use crate::server::State;

/// An output operation that cannot run.
#[derive(Debug, Error)]
pub enum OutputError {
    /// The output does not exist.
    #[error("no output {0}")]
    NoOutput(OutputId),
    /// Another output has the same connector name.
    #[error("an output named {0} exists")]
    DuplicateName(String),
    /// The last output cannot go.
    #[error("the last output cannot be removed")]
    LastOutput,
    /// No unplugged output has the name.
    #[error("no unplugged output named {0}")]
    NotUnplugged(String),
    /// No output was unplugged.
    #[error("no output was unplugged")]
    NothingUnplugged,
    /// The new configuration is invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The timing model rejects the change.
    #[error("timing: {0}")]
    Timing(#[from] fakeland_timing::ConfigError),
    /// Manual stepping failed.
    #[error("step: {0}")]
    Step(#[from] ScheduleError),
}

impl State {
    /// Adds an output and advertises its global.
    pub fn add_output(&mut self, config: OutputConfig) -> Result<OutputId, OutputError> {
        validate_output(&config)?;
        if self.outputs.by_name(&config.name).is_some() {
            return Err(OutputError::DuplicateName(config.name));
        }
        let id = self.ids.output();
        let position = self.outputs.place(&config);
        let now = self.now();
        let out = FakeOutput::new(id, config, position, &self.dh, now, |output, dh| {
            output.create_global::<State>(dh)
        })?;
        self.space.map_output(&out.output, (position.x, position.y));
        let info = out.info();
        let enabled = out.enabled();
        self.outputs.insert(out);
        self.bus.recorder_mut().header_mut().apply_output(&info);
        self.emit(Event::OutputConfigured(info));
        if !enabled {
            self.emit(Event::OutputPower {
                output: id,
                on: false,
            });
        }
        self.space_dirty = true;
        Ok(id)
    }

    /// Removes an output and its `wl_output` global. Its windows move to the primary output. The
    /// session keeps its config for a replug.
    pub fn remove_output(&mut self, id: OutputId) -> Result<(), OutputError> {
        if self.outputs.get(id).is_none() {
            return Err(OutputError::NoOutput(id));
        }
        if self.outputs.len() == 1 {
            return Err(OutputError::LastOutput);
        }
        let surfaces: Vec<_> = self
            .surfaces
            .iter()
            .filter(|s| s.output == Some(id))
            .map(|s| s.id)
            .collect();
        for s in surfaces {
            self.detach_surface(id, s);
            if let Some(rec) = self.surfaces.get_mut(s) {
                rec.output = None;
            }
        }
        self.close_output_layers(id);
        self.close_output_locks(id);
        self.close_output_gamma(id);
        let Some(out) = self.outputs.remove(id) else {
            return Err(OutputError::NoOutput(id));
        };
        let mut config = out.config.clone();
        config.position = Some(out.position);
        self.outputs.park(config);
        self.space.unmap_output(&out.output);
        self.dh.disable_global::<State>(out.global.clone());
        let dh = self.dh.clone();
        let global = out.global.clone();
        self.schedule_in(fakeland_types::Nanos::from_secs(1), move |_| {
            dh.remove_global::<State>(global);
        });
        let target = self.outputs.primary().ok_or(OutputError::LastOutput)?;
        for w in self.windows.on_output(id) {
            let _ = self.window_op(w, crate::shell::WindowOp::MoveToOutput { output: target });
        }
        self.emit(Event::OutputRemoved { output: id });
        self.space_dirty = true;
        Ok(())
    }

    /// Plugs an unplugged output in again with its name, identity, modes, scale, transform, timing
    /// and position. `None` selects the latest unplugged output. The output gets a new id.
    pub fn replug_output(&mut self, name: Option<&str>) -> Result<OutputId, OutputError> {
        let config = match (self.outputs.parked(name), name) {
            (Some(c), _) => c.clone(),
            (None, Some(name)) => return Err(OutputError::NotUnplugged(name.to_owned())),
            (None, None) => return Err(OutputError::NothingUnplugged),
        };
        let name = config.name.clone();
        let id = self.add_output(config)?;
        self.outputs.unpark(&name);
        Ok(id)
    }

    /// Applies one runtime change to an output.
    pub fn change_output(&mut self, id: OutputId, change: OutputChange) -> Result<(), OutputError> {
        let now = self.now();
        let out = self.outputs.get_mut(id).ok_or(OutputError::NoOutput(id))?;
        let mut relayout = false;
        match change {
            OutputChange::Mode(mode) => {
                let mut next = out.config.clone();
                next.mode = mode;
                validate_output(&next)?;
                let timing = next.effective_timing();
                if matches!(timing.mode, TimingMode::Fixed { .. }) {
                    out.scheduler.reconfigure(timing, now)?;
                }
                out.config = next;
                relayout = true;
            }
            OutputChange::Scale { scale } => {
                let mut next = out.config.clone();
                next.scale = scale;
                validate_output(&next)?;
                out.config = next;
                relayout = true;
            }
            OutputChange::Transform { transform } => {
                out.config.transform = transform;
                relayout = true;
            }
            OutputChange::Timing(timing) => {
                let mut next = out.config.clone();
                next.timing = timing;
                validate_output(&next)?;
                out.scheduler.reconfigure(next.effective_timing(), now)?;
                out.config = next;
            }
            OutputChange::Position(position) => {
                out.position = position;
                relayout = true;
            }
            OutputChange::Enable | OutputChange::Disable => {
                let on = matches!(change, OutputChange::Enable);
                if out.scheduler.is_enabled() != on {
                    out.scheduler.set_enabled(on, now);
                    out.config.enabled = on;
                    self.emit(Event::OutputPower { output: id, on });
                }
                return Ok(());
            }
        }
        let Some(out) = self.outputs.get(id) else {
            return Ok(());
        };
        out.sync_smithay_state();
        let (smithay_output, position, info) = (out.output.clone(), out.position, out.info());
        self.space
            .map_output(&smithay_output, (position.x, position.y));
        self.bus.recorder_mut().header_mut().apply_output(&info);
        self.emit(Event::OutputChanged(info));
        if relayout {
            self.relayout_output(id);
        }
        Ok(())
    }

    /// Arranges the layer surfaces of an output, resizes its lock surfaces and re-applies the
    /// placement and scale of every window on it.
    pub fn relayout_output(&mut self, id: OutputId) {
        self.arrange_layers(id);
        self.resize_output_locks(id);
        for l in self.layers.on_output(id) {
            if let Some(surface) = self.layers.get(l).map(|r| r.layer.wl_surface().clone()) {
                self.send_tree_scale(&surface);
            }
        }
        for w in self.windows.on_output(id) {
            self.relayout_window(w);
            if let Some(surface) = self.windows.get(w).map(|r| r.toplevel.wl_surface().clone()) {
                self.send_tree_scale(&surface);
            }
        }
        self.space_dirty = true;
    }

    /// Steps a manual output by `n` refreshes.
    pub fn step_output(&mut self, id: OutputId, n: u32) -> Result<(), OutputError> {
        let now = self.now();
        let out = self.outputs.get_mut(id).ok_or(OutputError::NoOutput(id))?;
        let actions = out.scheduler.step(n, now)?;
        self.execute_actions(id, actions);
        Ok(())
    }
}
