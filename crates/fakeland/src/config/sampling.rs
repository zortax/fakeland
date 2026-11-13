//! The configuration of resource sampling: the sample interval and the sources. Process samples
//! cover the process tree of every launched app; GPU samples cover the render device and the GPU
//! use of each tree; energy samples cover the CPU and GPU energy counters of the host.

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
    /// Sample proportional, private and swapped memory and the peak resident set of every tree.
    pub memory_detail: bool,
    /// Sample GPU memory and GPU busy time of every tree.
    pub gpu_process: bool,
    /// Sample the energy counters of the CPU packages and the GPU.
    pub energy: bool,
    /// The time between two reads of the slow GPU sources: the utilization of each tree and the
    /// GPU energy counter through NVML. Each of these reads stalls other GPU work of the host,
    /// such as a buffer allocation of an app or a readback of the compositor, for several
    /// milliseconds, so they run less often than the other sources. Their counters lose nothing
    /// between two reads.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub slow_interval: Nanos,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            interval: Nanos::from_millis(100),
            process: true,
            gpu: true,
            memory_detail: true,
            gpu_process: true,
            energy: true,
            slow_interval: Nanos::from_secs(1),
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

    /// Returns the settings with a time between two reads of the slow GPU sources.
    pub fn slow_interval(mut self, interval: Nanos) -> Self {
        self.slow_interval = interval;
        self
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

    /// Returns the settings with detailed memory sampling on or off.
    pub fn memory_detail(mut self, on: bool) -> Self {
        self.memory_detail = on;
        self
    }

    /// Returns the settings with per-tree GPU sampling on or off.
    pub fn gpu_process(mut self, on: bool) -> Self {
        self.gpu_process = on;
        self
    }

    /// Returns the settings with energy sampling on or off.
    pub fn energy(mut self, on: bool) -> Self {
        self.energy = on;
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
        assert!(c.memory_detail && c.gpu_process && c.energy);
        let c: SamplingConfig =
            serde_json::from_str(r#"{"memory_detail":false,"energy":false}"#).unwrap();
        assert!(!c.memory_detail && c.gpu_process && !c.energy);
        let back: SamplingConfig =
            serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn builders_set_fields() {
        let c = SamplingConfig::every(Nanos::from_millis(50))
            .process(false)
            .gpu(true)
            .memory_detail(false)
            .gpu_process(false)
            .energy(false);
        assert_eq!(c.interval, Nanos::from_millis(50));
        assert!(!c.process && c.gpu);
        assert!(!c.memory_detail && !c.gpu_process && !c.energy);
        let on = c.memory_detail(true).gpu_process(true).energy(true);
        assert!(on.memory_detail && on.gpu_process && on.energy);
    }
}
