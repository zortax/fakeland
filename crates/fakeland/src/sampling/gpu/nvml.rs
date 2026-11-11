//! GPU samples through NVML. The library loads once per sampler, and the device is the NVML device
//! with the PCI address of the render node, or the first device. The device-wide probe, the
//! per-tree probe and the energy meter share the loaded library.

use std::sync::Arc;

use nvml_wrapper::{Device, Nvml};

use super::device::same_pci;
use super::{GpuProbe, GpuReading, RenderDevice};

pub use super::nvml_process::NvmlProcessProbe;

/// One NVML device of a loaded library.
#[derive(Clone)]
pub struct NvmlDevice {
    nvml: Arc<Nvml>,
    index: u32,
}

impl std::fmt::Debug for NvmlDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NvmlDevice")
            .field("index", &self.index)
            .finish()
    }
}

impl NvmlDevice {
    /// Loads NVML and finds the device. Returns `None` when NVML does not load or has no device.
    pub fn open(device: Option<&RenderDevice>) -> Option<Self> {
        let nvml = match Nvml::init() {
            Ok(n) => n,
            Err(e) => {
                tracing::info!("NVML is unavailable: {e}");
                return None;
            }
        };
        let count = nvml.device_count().ok()?;
        let wanted = device.and_then(|d| d.pci.clone());
        let index = (0..count)
            .find(|&i| {
                let Some(pci) = &wanted else { return true };
                nvml.device_by_index(i)
                    .and_then(|d| d.pci_info())
                    .is_ok_and(|info| same_pci(&info.bus_id, pci))
            })
            .or_else(|| (count > 0).then_some(0))?;
        Some(Self {
            nvml: Arc::new(nvml),
            index,
        })
    }

    /// Returns the device index.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns the device handle, or `None` when NVML loses the device.
    pub fn device(&self) -> Option<Device<'_>> {
        self.nvml.device_by_index(self.index).ok()
    }

    /// Returns the energy of the device since the driver loaded, in millijoules.
    pub fn energy_mj(&self) -> Option<u64> {
        self.device()?.total_energy_consumption().ok()
    }
}

/// Reads utilization and memory of one NVIDIA GPU.
#[derive(Debug)]
pub struct NvmlProbe {
    dev: NvmlDevice,
}

impl NvmlProbe {
    /// Builds the probe of a device.
    pub fn new(dev: NvmlDevice) -> Self {
        Self { dev }
    }
}

impl GpuProbe for NvmlProbe {
    fn name(&self) -> &'static str {
        "nvml"
    }

    fn read(&mut self) -> Option<GpuReading> {
        let device = self.dev.device()?;
        let util = device.utilization_rates().ok()?;
        let mem = device.memory_info().ok()?;
        Some(GpuReading {
            util_percent: f64::from(util.gpu),
            mem_used_bytes: mem.used,
            mem_total_bytes: mem.total,
        })
    }
}
