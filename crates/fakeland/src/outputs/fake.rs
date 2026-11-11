//! One faked output: the smithay `Output` with its `wl_output` global, the display scheduler and
//! the configuration it came from.

use fakeland_scenario::Transform;
use fakeland_timing::{Scheduler, TimingMode};
use fakeland_trace::OutputInfo;
use fakeland_types::{Logical, OutputId, Rect, Timestamp};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::utils::Transform as SmithayTransform;

use super::OutputState;
use crate::config::{ModeConfig, OutputConfig, OutputPosition};

/// A faked output.
#[derive(Debug)]
pub struct FakeOutput {
    /// The output id.
    pub id: OutputId,
    /// The current configuration.
    pub config: OutputConfig,
    /// The smithay output.
    pub output: Output,
    /// The `wl_output` global.
    pub global: GlobalId,
    /// The display scheduler.
    pub scheduler: Scheduler,
    /// The position in the global space.
    pub position: OutputPosition,
}

impl FakeOutput {
    /// Creates the output and its global. The scheduler grid starts at `now`.
    pub fn new(
        id: OutputId,
        config: OutputConfig,
        position: OutputPosition,
        dh: &DisplayHandle,
        now: Timestamp,
        global: impl FnOnce(&Output, &DisplayHandle) -> GlobalId,
    ) -> Result<Self, fakeland_timing::ConfigError> {
        let mut scheduler = Scheduler::new(config.effective_timing(), now)?;
        if !config.enabled {
            scheduler.set_enabled(false, now);
        }
        let (mm_w, mm_h) = config
            .physical
            .millimeters(config.mode.width, config.mode.height);
        let output = Output::new(
            config.name.clone(),
            PhysicalProperties {
                size: (mm_w, mm_h).into(),
                subpixel: Subpixel::Unknown,
                make: config.make.clone(),
                model: config.model.clone(),
            },
        );
        for mode in &config.extra_modes {
            output.add_mode(smithay_mode(mode));
        }
        let current = smithay_mode(&config.mode);
        output.set_preferred(current);
        output.change_current_state(
            Some(current),
            Some(smithay_transform(config.transform)),
            Some(Scale::Fractional(config.scale)),
            Some((position.x, position.y).into()),
        );
        let global = global(&output, dh);
        Ok(Self {
            id,
            config,
            output,
            global,
            scheduler,
            position,
        })
    }

    /// Returns the output rectangle in the global logical space.
    pub fn logical_rect(&self) -> Rect<Logical> {
        let size = self.config.logical_size();
        Rect::new(self.position.x, self.position.y, size.w, size.h)
    }

    /// Returns true when the output is powered on.
    pub fn enabled(&self) -> bool {
        self.scheduler.is_enabled()
    }

    /// Pushes the current mode, scale, transform and position to the smithay output, which sends
    /// the changes to every bound client.
    pub fn sync_smithay_state(&self) {
        let current = smithay_mode(&self.config.mode);
        self.output.change_current_state(
            Some(current),
            Some(smithay_transform(self.config.transform)),
            Some(Scale::Fractional(self.config.scale)),
            Some((self.position.x, self.position.y).into()),
        );
    }

    /// Returns the trace description of the output.
    pub fn info(&self) -> OutputInfo {
        OutputInfo {
            output: self.id,
            name: self.config.name.clone(),
            width: self.config.mode.width,
            height: self.config.mode.height,
            refresh_mhz: match self.scheduler.config().mode {
                TimingMode::Fixed { refresh } => refresh.millihertz().max(0) as u32,
                _ => 0,
            },
            scale: self.config.scale,
            transform: transform_name(self.config.transform).to_string(),
            timing: timing_label(&self.scheduler.config().mode),
        }
    }

    /// Returns the public view of the output.
    pub fn state(&self) -> OutputState {
        let mut modes = vec![self.config.mode];
        modes.extend(self.config.extra_modes.iter().copied());
        OutputState {
            id: self.id,
            name: self.config.name.clone(),
            make: self.config.make.clone(),
            model: self.config.model.clone(),
            serial: self.config.serial.clone(),
            mode: self.config.mode,
            modes,
            scale: self.config.scale,
            transform: self.config.transform,
            position: self.position,
            logical_size: self.config.logical_size(),
            timing: *self
                .scheduler
                .pending_config()
                .unwrap_or(self.scheduler.config()),
            enabled: self.enabled(),
            physical_mm: self
                .config
                .physical
                .millimeters(self.config.mode.width, self.config.mode.height),
        }
    }
}

/// Converts a mode to a smithay mode.
pub fn smithay_mode(mode: &ModeConfig) -> Mode {
    Mode {
        size: (mode.width, mode.height).into(),
        refresh: mode.refresh.millihertz(),
    }
}

/// Converts a transform to the smithay transform.
pub fn smithay_transform(t: Transform) -> SmithayTransform {
    match t {
        Transform::Normal => SmithayTransform::Normal,
        Transform::Rotate90 => SmithayTransform::_90,
        Transform::Rotate180 => SmithayTransform::_180,
        Transform::Rotate270 => SmithayTransform::_270,
        Transform::Flipped => SmithayTransform::Flipped,
        Transform::Flipped90 => SmithayTransform::Flipped90,
        Transform::Flipped180 => SmithayTransform::Flipped180,
        Transform::Flipped270 => SmithayTransform::Flipped270,
    }
}

/// Returns the trace name of a transform.
pub fn transform_name(t: Transform) -> &'static str {
    match t {
        Transform::Normal => "normal",
        Transform::Rotate90 => "90",
        Transform::Rotate180 => "180",
        Transform::Rotate270 => "270",
        Transform::Flipped => "flipped",
        Transform::Flipped90 => "flipped_90",
        Transform::Flipped180 => "flipped_180",
        Transform::Flipped270 => "flipped_270",
    }
}

/// Returns a short label of a timing mode for the trace.
pub fn timing_label(mode: &TimingMode) -> String {
    match mode {
        TimingMode::Fixed { refresh } => format!("fixed {:.3} Hz", refresh.0),
        TimingMode::Vrr { min, max } => format!("vrr {:.3}-{:.3} Hz", min.0, max.0),
        TimingMode::Unthrottled => "unthrottled".into(),
        TimingMode::Manual { nominal } => format!("manual {:.3} Hz", nominal.0),
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Hz;

    use super::*;

    #[test]
    fn labels_timing_modes() {
        assert_eq!(
            timing_label(&TimingMode::fixed(Hz(144.0))),
            "fixed 144.000 Hz"
        );
        assert_eq!(timing_label(&TimingMode::Unthrottled), "unthrottled");
        assert_eq!(
            timing_label(&TimingMode::vrr(Hz(48.0), Hz(165.0))),
            "vrr 48.000-165.000 Hz"
        );
    }

    #[test]
    fn transforms_map_one_to_one() {
        let all = [
            Transform::Normal,
            Transform::Rotate90,
            Transform::Rotate180,
            Transform::Rotate270,
            Transform::Flipped,
            Transform::Flipped90,
            Transform::Flipped180,
            Transform::Flipped270,
        ];
        for t in all {
            let wl: u32 = smithay::reexports::wayland_server::protocol::wl_output::Transform::from(
                smithay_transform(t),
            )
            .into();
            assert_eq!(wl, t.wayland_value());
        }
    }
}
