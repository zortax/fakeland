//! Resource sampling: CPU, memory, context switches and threads of every launched process tree
//! from `/proc`, GPU use of the render device and of each tree, and the energy counters of the
//! host. A sampler thread reads the sources at each interval, and the compositor thread records
//! the samples in the trace.

mod collect;
mod cpu;
mod energy;
mod gate;
mod gpu;
mod procfs;
mod sampler;
mod service;
mod smaps;
mod snapshot;
mod switches;
mod tree;

pub use energy::EnergyReading;
pub use gate::GpuGate;
pub use gpu::GpuReading;
pub use service::Sampling;
pub use snapshot::{ProcessTreeSample, ResourceSnapshot};
