//! GPU use of each tree through NVML. Memory is the used GPU memory of each member in the graphics
//! and compute process lists. Utilization is the mean SM utilization of the samples that the
//! driver buffered for each member since the previous read, summed over members. NVML counts no
//! busy time, so the readings carry utilization and no busy time.

use std::collections::HashMap;

use fakeland_types::Timestamp;
use nvml_wrapper::enums::device::UsedGpuMemory;
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::struct_wrappers::device::ProcessInfo;

use super::nvml::NvmlDevice;
use super::process::{GpuProcessProbe, GpuProcessReading, TreeMembers};

/// Reads the GPU memory and utilization of tree members through NVML.
#[derive(Debug)]
pub struct NvmlProcessProbe {
    dev: NvmlDevice,
    last_seen: Option<u64>,
}

impl NvmlProcessProbe {
    /// Builds the probe and skips the utilization samples before now. Returns `None` when the
    /// device has no process list.
    pub fn open(dev: NvmlDevice) -> Option<Self> {
        dev.device()?.running_graphics_processes_count().ok()?;
        let mut probe = Self {
            dev,
            last_seen: None,
        };
        probe.utilization();
        Some(probe)
    }

    /// Returns the used memory of each listed process in bytes.
    fn memory(&self) -> HashMap<u32, u64> {
        let Some(device) = self.dev.device() else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        let lists = [
            device.running_graphics_processes(),
            device.running_compute_processes(),
        ];
        for p in lists.into_iter().flat_map(Result::unwrap_or_default) {
            let ProcessInfo {
                pid,
                used_gpu_memory: UsedGpuMemory::Used(bytes),
                ..
            } = p
            else {
                out.entry(p.pid).or_insert(0);
                continue;
            };
            let e = out.entry(pid).or_insert(0);
            *e = (*e).max(bytes);
        }
        out
    }

    /// Returns the mean SM utilization of each process since the previous call, or `None` when
    /// the device has no process utilization.
    fn utilization(&mut self) -> Option<HashMap<u32, f64>> {
        let device = self.dev.device()?;
        let samples = match device.process_utilization_stats(self.last_seen) {
            Ok(s) => s,
            Err(NvmlError::NotFound) => Vec::new(),
            Err(e) => {
                tracing::debug!("NVML process utilization is unavailable: {e}");
                return None;
            }
        };
        let mut sums: HashMap<u32, (f64, u32)> = HashMap::new();
        for s in &samples {
            if self.last_seen.is_some_and(|t| s.timestamp <= t) {
                continue;
            }
            let e = sums.entry(s.pid).or_default();
            e.0 += f64::from(s.sm_util);
            e.1 += 1;
        }
        if let Some(t) = samples.iter().map(|s| s.timestamp).max() {
            self.last_seen = Some(self.last_seen.map_or(t, |l| l.max(t)));
        }
        Some(
            sums.into_iter()
                .map(|(pid, (sum, n))| (pid, sum / f64::from(n)))
                .collect(),
        )
    }
}

impl GpuProcessProbe for NvmlProcessProbe {
    fn name(&self) -> &'static str {
        "nvml"
    }

    fn read(&mut self, _: Timestamp, trees: &[TreeMembers<'_>]) -> Vec<GpuProcessReading> {
        let memory = self.memory();
        let util = self.utilization();
        let mut out = Vec::new();
        for tree in trees {
            let listed: Vec<u64> = tree
                .pids
                .iter()
                .filter_map(|p| memory.get(p).copied())
                .collect();
            let busy: f64 = util
                .iter()
                .flat_map(|u| tree.pids.iter().filter_map(|p| u.get(p)))
                .sum();
            if listed.is_empty() && busy <= 0.0 {
                continue;
            }
            out.push(GpuProcessReading {
                root: tree.root,
                mem_bytes: (!listed.is_empty()).then(|| listed.iter().sum()),
                busy_ns_total: None,
                util_percent: util.as_ref().map(|_| busy),
                source: "nvml".into(),
            });
        }
        out
    }

    fn forget(&mut self, _: u32) {}
}
