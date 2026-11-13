//! The gate between the slow GPU reads of the sampler thread and GPU work of the compositor
//! thread. The NVIDIA driver serializes both: while NVML reads the utilization of the processes
//! or the energy counter, a GL call of the compositor that allocates or frees can block for
//! several milliseconds. The sampler holds the gate during these reads. The compositor thread
//! never waits for the gate: work that can wait, such as a probe readback, tries it and runs
//! later while a read holds it.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

/// The gate. Clones share it.
#[derive(Clone, Debug, Default)]
pub struct GpuGate(Arc<Mutex<()>>);

/// Holds the gate until dropped.
#[derive(Debug)]
pub struct GpuGateGuard<'a>(#[allow(dead_code)] MutexGuard<'a, ()>);

impl GpuGate {
    /// Waits for the gate. For the sampler thread only.
    pub fn enter(&self) -> GpuGateGuard<'_> {
        GpuGateGuard(self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Takes the gate when it is free. Returns `None` while the other side holds it.
    pub fn try_enter(&self) -> Option<GpuGateGuard<'_>> {
        match self.0.try_lock() {
            Ok(guard) => Some(GpuGateGuard(guard)),
            Err(TryLockError::Poisoned(e)) => Some(GpuGateGuard(e.into_inner())),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_admits_one_side_at_a_time() {
        let gate = GpuGate::default();
        let other = gate.clone();
        let held = gate.enter();
        assert!(other.try_enter().is_none());
        drop(held);
        assert!(other.try_enter().is_some());
        assert!(gate.try_enter().is_some());
    }
}
