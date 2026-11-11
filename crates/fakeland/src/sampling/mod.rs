//! Resource sampling: CPU, resident memory and threads of every launched process tree from
//! `/proc`, and GPU utilization and memory of the render device. A sampler thread reads the
//! sources at each interval, and the compositor thread records the samples in the trace.

mod cpu;
mod gpu;
mod procfs;
mod sampler;
mod service;
mod snapshot;
mod tree;

pub use gpu::GpuReading;
pub use service::Sampling;
pub use snapshot::{ProcessTreeSample, ResourceSnapshot};
