//! The configuration of one faked output: connector name, monitor identity, modes, scale,
//! transform, physical size, position and display timing.

use fakeland_scenario::Transform;
use fakeland_timing::{TimingConfig, TimingMode};
use fakeland_types::{Hz, Logical, Size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One display mode: a pixel size and a refresh rate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModeConfig {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// The refresh rate that `wl_output.mode` advertises.
    pub refresh: Hz,
}

impl ModeConfig {
    /// Builds a mode.
    pub const fn new(width: i32, height: i32, refresh: Hz) -> Self {
        Self {
            width,
            height,
            refresh,
        }
    }
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self::new(1920, 1080, Hz(60.0))
    }
}

/// The physical size of the faked monitor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PhysicalSize {
    /// An explicit size in millimeters.
    Millimeters {
        /// Width in millimeters.
        width: i32,
        /// Height in millimeters.
        height: i32,
    },
    /// A pixel density. The size follows from the current mode.
    Dpi {
        /// Pixels per inch.
        dpi: f64,
    },
}

impl Default for PhysicalSize {
    fn default() -> Self {
        Self::Dpi { dpi: 92.0 }
    }
}

impl PhysicalSize {
    /// Returns the size in millimeters for a mode of `width` x `height` pixels.
    pub fn millimeters(self, width: i32, height: i32) -> (i32, i32) {
        match self {
            Self::Millimeters { width, height } => (width, height),
            Self::Dpi { dpi } => {
                let mm = |px: i32| (px as f64 / dpi * 25.4).round() as i32;
                (mm(width), mm(height))
            }
        }
    }
}

/// A position in the global logical compositor space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutputPosition {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
}

/// The configuration of one output.
///
/// In fixed timing mode the output refreshes at the rate of the current mode. VRR, manual and
/// unthrottled timing modes keep their own rates, and the mode rate is only advertised.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct OutputConfig {
    /// The connector name, for example `DP-1`.
    pub name: String,
    /// The monitor manufacturer.
    pub make: String,
    /// The monitor model.
    pub model: String,
    /// The monitor serial number.
    pub serial: String,
    /// The current mode.
    pub mode: ModeConfig,
    /// More advertised modes.
    pub extra_modes: Vec<ModeConfig>,
    /// The output scale. Fractional scales are allowed.
    pub scale: f64,
    /// The output transform.
    pub transform: Transform,
    /// The physical size.
    pub physical: PhysicalSize,
    /// The position in the global space. `None` places the output right of the previous one.
    pub position: Option<OutputPosition>,
    /// The display timing model.
    pub timing: TimingConfig,
    /// The output starts powered on.
    pub enabled: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            name: "DP-1".into(),
            make: "Dell Inc.".into(),
            model: "DELL P2422H".into(),
            serial: "7Z6X3G3".into(),
            mode: ModeConfig::default(),
            extra_modes: Vec::new(),
            scale: 1.0,
            transform: Transform::Normal,
            physical: PhysicalSize::default(),
            position: None,
            timing: TimingConfig::default(),
            enabled: true,
        }
    }
}

impl OutputConfig {
    /// Builds an output with a connector name and the default identity.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Sets the current mode.
    pub fn mode(mut self, width: i32, height: i32, refresh: Hz) -> Self {
        self.mode = ModeConfig::new(width, height, refresh);
        self
    }

    /// Sets the scale.
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the transform.
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the monitor identity.
    pub fn identity(
        mut self,
        make: impl Into<String>,
        model: impl Into<String>,
        serial: impl Into<String>,
    ) -> Self {
        self.make = make.into();
        self.model = model.into();
        self.serial = serial.into();
        self
    }

    /// Sets the physical size.
    pub fn physical(mut self, physical: PhysicalSize) -> Self {
        self.physical = physical;
        self
    }

    /// Sets the position in the global space.
    pub fn position(mut self, x: i32, y: i32) -> Self {
        self.position = Some(OutputPosition { x, y });
        self
    }

    /// Sets the timing model.
    pub fn timing(mut self, timing: TimingConfig) -> Self {
        self.timing = timing;
        self
    }

    /// Adds an advertised mode.
    pub fn extra_mode(mut self, mode: ModeConfig) -> Self {
        self.extra_modes.push(mode);
        self
    }

    /// Sets whether the output starts powered on.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the timing model with the fixed rate taken from the current mode.
    pub fn effective_timing(&self) -> TimingConfig {
        let mut timing = self.timing;
        if let TimingMode::Fixed { refresh } = &mut timing.mode {
            *refresh = self.mode.refresh;
        }
        timing
    }

    /// Returns the logical size: the mode size, rotated by the transform, divided by the scale.
    pub fn logical_size(&self) -> Size<Logical> {
        logical_size(
            self.mode.width,
            self.mode.height,
            self.scale,
            self.transform,
        )
    }
}

/// Returns the logical size of a mode under a scale and a transform.
pub fn logical_size(width: i32, height: i32, scale: f64, transform: Transform) -> Size<Logical> {
    let (w, h) = match transform {
        Transform::Rotate90
        | Transform::Rotate270
        | Transform::Flipped90
        | Transform::Flipped270 => (height, width),
        _ => (width, height),
    };
    Size::new(
        (w as f64 / scale).round() as i32,
        (h as f64 / scale).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_size_follows_scale_and_rotation() {
        assert_eq!(
            logical_size(2560, 1440, 1.5, Transform::Normal),
            Size::new(1707, 960)
        );
        assert_eq!(
            logical_size(1920, 1080, 2.0, Transform::Rotate90),
            Size::new(540, 960)
        );
        assert_eq!(
            logical_size(1920, 1080, 1.0, Transform::Flipped180),
            Size::new(1920, 1080)
        );
    }

    #[test]
    fn dpi_gives_millimeters() {
        let mm = PhysicalSize::Dpi { dpi: 96.0 }.millimeters(1920, 1080);
        assert_eq!(mm, (508, 286));
        let fixed = PhysicalSize::Millimeters {
            width: 600,
            height: 340,
        };
        assert_eq!(fixed.millimeters(1, 1), (600, 340));
    }

    #[test]
    fn fixed_timing_follows_the_mode() {
        let out = OutputConfig::new("DP-2").mode(2560, 1440, Hz(144.0));
        assert_eq!(
            out.effective_timing().mode,
            TimingMode::Fixed { refresh: Hz(144.0) }
        );
        let vrr = out.clone().timing(TimingConfig::vrr(Hz(48.0), Hz(144.0)));
        assert_eq!(
            vrr.effective_timing().mode,
            TimingMode::Vrr {
                min: Hz(48.0),
                max: Hz(144.0)
            }
        );
    }

    #[test]
    fn output_config_round_trips() {
        let out = OutputConfig::new("HDMI-A-1")
            .mode(3840, 2160, Hz(59.94))
            .scale(1.5)
            .position(100, 0);
        let json = serde_json::to_string(&out).unwrap();
        let back: OutputConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, out);
        let short: OutputConfig = serde_json::from_str(r#"{"name":"DP-3","scale":2}"#).unwrap();
        assert_eq!(short.name, "DP-3");
        assert_eq!(short.scale, 2.0);
        assert_eq!(short.mode, ModeConfig::default());
    }
}
