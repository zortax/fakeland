//! One GPU reading: utilization and memory of the render device.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// GPU utilization and memory at one sample.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GpuReading {
    /// Utilization in percent over the last sample period of the driver.
    pub util_percent: f64,
    /// Used device memory in bytes.
    pub mem_used_bytes: u64,
    /// Total device memory in bytes.
    pub mem_total_bytes: u64,
}
