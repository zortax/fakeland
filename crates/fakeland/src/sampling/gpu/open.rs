//! The GPU probe interface, the GPU context of a sampler and the choice of a probe for the render
//! device.

use std::path::Path;

use super::{GpuReading, RenderDevice};

/// A source of GPU readings.
pub trait GpuProbe: Send + std::fmt::Debug {
    /// Returns the source name, for example `nvml` or `sysfs`.
    fn name(&self) -> &'static str;

    /// Reads the current utilization and memory, or `None` when the read fails.
    fn read(&mut self) -> Option<GpuReading>;
}

/// The render device and, with the `nvml` feature, the NVML device of a sampler.
#[derive(Debug, Default)]
pub struct GpuContext {
    /// The sysfs view of the render device.
    pub device: Option<RenderDevice>,
    /// The NVML device of an NVIDIA render device, or of the first NVIDIA GPU without a render
    /// device.
    #[cfg(feature = "nvml")]
    pub nvml: Option<super::nvml::NvmlDevice>,
}

impl GpuContext {
    /// Finds the device of a render node. Loads NVML only when `nvml` is true and the device is
    /// an NVIDIA GPU or unknown.
    pub fn open(node: Option<&Path>, nvml: bool) -> Self {
        let device = node.and_then(RenderDevice::of_node);
        #[cfg(feature = "nvml")]
        let nvml = (nvml && device.as_ref().is_none_or(RenderDevice::is_nvidia))
            .then(|| super::nvml::NvmlDevice::open(device.as_ref()))
            .flatten();
        #[cfg(not(feature = "nvml"))]
        let _ = nvml;
        Self {
            device,
            #[cfg(feature = "nvml")]
            nvml,
        }
    }
}

/// Opens a probe for the render device. NVIDIA GPUs need the `nvml` feature; other GPUs need
/// sysfs counters. Returns `None` when no source works.
pub fn open_probe(ctx: &GpuContext) -> Option<Box<dyn GpuProbe>> {
    #[cfg(feature = "nvml")]
    if let Some(dev) = ctx.nvml.clone() {
        return Some(Box::new(super::nvml::NvmlProbe::new(dev)));
    }
    let device = ctx.device.as_ref()?;
    if let Some(p) = super::sysfs::SysfsProbe::open(&device.dir) {
        return Some(Box::new(p));
    }
    if device.is_nvidia() && !cfg!(feature = "nvml") {
        tracing::info!("GPU samples of NVIDIA GPUs need the nvml feature");
    }
    None
}
