//! The configuration of resource sampling: the sample interval and the sources. Process samples
//! cover the process tree of every launched app; GPU samples cover the render device.

use fakeland_types::Nanos;
use fakeland_types::dur::{Dur, as_dur};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The shortest sample interval.
pub const MIN_SAMPLE_INTERVAL: Nanos = Nanos::from_millis(10);

/// Resource sampling settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SamplingConfig {
    /// The time between two samples. The shortest interval is 10 ms.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub interval: Nanos,
    /// Sample CPU, resident memory and threads of every launched process tree.
    pub process: bool,
    /// Sample GPU utilization and memory of the render device.
    pub gpu: bool,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            interval: Nanos::from_millis(100),
            process: true,
            gpu: true,
        }
    }
}

impl SamplingConfig {
    /// Samples processes and the GPU at an interval.
    pub fn every(interval: Nanos) -> Self {
        Self {
            interval,
            ..Self::default()
        }
    }

    /// Returns the settings with process sampling on or off.
    pub fn process(mut self, on: bool) -> Self {
        self.process = on;
        self
    }

    /// Returns the settings with GPU sampling on or off.
    pub fn gpu(mut self, on: bool) -> Self {
        self.gpu = on;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_defaults_and_round_trip() {
        let c: SamplingConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c, SamplingConfig::default());
        let c: SamplingConfig =
            serde_json::from_str(r#"{"interval":"250ms","gpu":false}"#).unwrap();
        assert_eq!(c.interval, Nanos::from_millis(250));
        assert!(c.process && !c.gpu);
        let back: SamplingConfig =
            serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn builders_set_fields() {
        let c = SamplingConfig::every(Nanos::from_millis(50))
            .process(false)
            .gpu(true);
        assert_eq!(c.interval, Nanos::from_millis(50));
        assert!(!c.process && c.gpu);
    }
}
