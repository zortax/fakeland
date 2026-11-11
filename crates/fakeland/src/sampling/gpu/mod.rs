//! GPU samples: device-wide utilization and memory of the render device, and the GPU use of each
//! process tree. NVML serves NVIDIA GPUs with the `nvml` feature; sysfs counters and DRM fdinfo
//! serve drivers that publish them. Sampling goes on without GPU samples when no source works.

mod device;
mod fdinfo;
#[cfg(feature = "nvml")]
mod nvml;
#[cfg(feature = "nvml")]
mod nvml_process;
mod open;
mod process;
mod reading;
mod sysfs;
mod usage;

pub use device::RenderDevice;
#[cfg(feature = "nvml")]
pub use nvml::NvmlDevice;
pub use open::{GpuContext, GpuProbe, open_probe};
pub use process::{GpuProcessChain, GpuProcessReading, TreeMembers};
pub use reading::GpuReading;
