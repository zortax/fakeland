//! GPU samples of the render device: NVML for NVIDIA GPUs with the `nvml` feature, and the
//! sysfs counters of drivers that publish them. Sampling goes on without GPU samples when no
//! source works.

mod device;
#[cfg(feature = "nvml")]
mod nvml;
mod open;
mod reading;
mod sysfs;

pub use device::RenderDevice;
pub use open::{GpuProbe, open_probe};
pub use reading::GpuReading;
