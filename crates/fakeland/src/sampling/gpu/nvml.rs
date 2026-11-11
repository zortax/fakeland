//! GPU samples through NVML. The library loads at probe start, and the device is the NVML device
//! with the PCI address of the render node, or the first device.

use nvml_wrapper::Nvml;

use super::device::same_pci;
use super::{GpuProbe, GpuReading, RenderDevice};

/// Reads utilization and memory of one NVIDIA GPU.
pub struct NvmlProbe {
    nvml: Nvml,
    index: u32,
}

impl std::fmt::Debug for NvmlProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NvmlProbe")
            .field("index", &self.index)
            .finish()
    }
}

impl NvmlProbe {
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
        Some(Self { nvml, index })
    }
}

impl GpuProbe for NvmlProbe {
    fn name(&self) -> &'static str {
        "nvml"
    }

    fn read(&mut self) -> Option<GpuReading> {
        let device = self.nvml.device_by_index(self.index).ok()?;
        let util = device.utilization_rates().ok()?;
        let mem = device.memory_info().ok()?;
        Some(GpuReading {
            util_percent: f64::from(util.gpu),
            mem_used_bytes: mem.used,
            mem_total_bytes: mem.total,
        })
    }
}
