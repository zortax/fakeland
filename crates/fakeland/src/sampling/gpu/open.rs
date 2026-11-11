//! The GPU probe interface and the choice of a probe for the render device.

use std::path::Path;

use super::{GpuReading, RenderDevice};

/// A source of GPU readings.
pub trait GpuProbe: Send + std::fmt::Debug {
    /// Returns the source name, for example `nvml` or `sysfs`.
    fn name(&self) -> &'static str;

    /// Reads the current utilization and memory, or `None` when the read fails.
    fn read(&mut self) -> Option<GpuReading>;
}

/// Opens a probe for a render node. NVIDIA GPUs need the `nvml` feature; other GPUs need sysfs
/// counters. Returns `None` when no source works.
pub fn open_probe(node: Option<&Path>) -> Option<Box<dyn GpuProbe>> {
    let device = node.and_then(RenderDevice::of_node);
    #[cfg(feature = "nvml")]
    if device.as_ref().is_none_or(RenderDevice::is_nvidia)
        && let Some(p) = super::nvml::NvmlProbe::open(device.as_ref())
    {
        return Some(Box::new(p));
    }
    let device = device?;
    if let Some(p) = super::sysfs::SysfsProbe::open(&device.dir) {
        return Some(Box::new(p));
    }
    if device.is_nvidia() && !cfg!(feature = "nvml") {
        tracing::info!("GPU samples of NVIDIA GPUs need the nvml feature");
    }
    None
}
