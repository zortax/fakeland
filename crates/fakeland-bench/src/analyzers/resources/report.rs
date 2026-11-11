//! The resources report.

use fakeland_trace::Summary;
use fakeland_types::ProcessId;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The samples of one process tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessResources {
    /// The root process.
    pub pid: ProcessId,
    /// The command line, when the trace has its spawn.
    pub command: Option<String>,
    /// The count of samples.
    pub samples: usize,
    /// CPU use in percent of one CPU.
    pub cpu_percent: Option<Summary>,
    /// The largest resident set.
    pub rss_max_bytes: u64,
    /// The mean resident set.
    pub rss_mean_bytes: f64,
    /// The largest thread count.
    pub threads_max: u32,
    /// The time on CPU from the first to the last sample of the iteration.
    #[serde(default)]
    pub cpu_ms: Option<f64>,
    /// The time on CPU per second of wall time.
    #[serde(default)]
    pub cpu_ms_per_s: Option<f64>,
    /// Context switches per second.
    #[serde(default)]
    pub wakeups_per_s: Option<f64>,
    /// The largest proportional set size.
    #[serde(default)]
    pub pss_max_bytes: Option<u64>,
    /// The largest private memory.
    #[serde(default)]
    pub private_max_bytes: Option<u64>,
    /// The largest swapped memory.
    #[serde(default)]
    pub swap_max_bytes: Option<u64>,
    /// The largest peak resident set of a member.
    #[serde(default)]
    pub hwm_max_bytes: Option<u64>,
    /// The largest GPU memory of the tree.
    #[serde(default)]
    pub gpu_mem_max_bytes: Option<u64>,
    /// The GPU busy share of the tree in percent: busy time over wall time, or the mean
    /// utilization of a source without busy time.
    #[serde(default)]
    pub gpu_busy_percent: Option<f64>,
    /// The source of the GPU values of the tree.
    #[serde(default)]
    pub gpu_source: Option<String>,
}

/// The GPU samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpuResources {
    /// The count of samples.
    pub samples: usize,
    /// GPU use in percent.
    pub util_percent: Option<Summary>,
    /// The largest used memory.
    pub mem_used_max_bytes: u64,
    /// The memory of the device.
    pub mem_total_bytes: u64,
}

/// The energy of one device domain over the iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnergyDomain {
    /// The domain, for example `cpu-package-0` or `gpu-nvml-0`.
    pub domain: String,
    /// The energy from the first to the last sample.
    pub joules: f64,
    /// The mean power.
    pub watts: Option<f64>,
}

/// Resource use over an iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourcesReport {
    /// Every sampled process tree, by root pid.
    pub processes: Vec<ProcessResources>,
    /// The GPU, when the trace has GPU samples.
    pub gpu: Option<GpuResources>,
    /// The trees that the process metrics cover: the tree of the target, else every tree.
    #[serde(default)]
    pub selected: Vec<ProcessId>,
    /// Every energy domain. Energy is device-wide and covers every process of the host.
    #[serde(default)]
    pub energy: Vec<EnergyDomain>,
    /// The mean power of the CPU package domains.
    #[serde(default)]
    pub cpu_power_w: Option<f64>,
    /// The mean power of the GPU domains.
    #[serde(default)]
    pub gpu_power_w: Option<f64>,
}

impl ResourcesReport {
    /// Returns the trees that the process metrics cover.
    pub fn selected_processes(&self) -> impl Iterator<Item = &ProcessResources> + '_ {
        self.processes
            .iter()
            .filter(|p| self.selected.contains(&p.pid))
    }
}

/// Returns the sum of the present values, or `None` when no value is present.
fn sum<T: std::iter::Sum<T>>(values: impl Iterator<Item = Option<T>>) -> Option<T> {
    let present: Vec<T> = values.flatten().collect();
    (!present.is_empty()).then(|| present.into_iter().sum())
}

impl Metrics for ResourcesReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        let procs: Vec<&ProcessResources> = self.selected_processes().collect();
        if procs.iter().any(|p| p.samples > 0) {
            let cpu: f64 = procs.iter().filter_map(|p| p.cpu_percent.mean()).sum();
            let rss: u64 = procs.iter().map(|p| p.rss_max_bytes).sum();
            let threads: u32 = procs.iter().map(|p| p.threads_max).sum();
            m.lower("cpu_percent_mean", Some(cpu), Unit::Percent);
            m.lower("rss_max", Some(rss as f64), Unit::Bytes);
            m.neutral("threads_max", Some(f64::from(threads)), Unit::Count);
        }
        if let Some(g) = &self.gpu {
            m.lower("gpu_util_mean", g.util_percent.mean(), Unit::Percent);
            m.lower(
                "gpu_mem_max",
                Some(g.mem_used_max_bytes as f64),
                Unit::Bytes,
            );
        }
        let bytes = |v: Option<u64>| v.map(|b| b as f64);
        m.lower(
            "cpu_ms_per_s",
            sum(procs.iter().map(|p| p.cpu_ms_per_s)),
            Unit::MsPerS,
        );
        m.lower("cpu_ms", sum(procs.iter().map(|p| p.cpu_ms)), Unit::Ms);
        m.lower(
            "wakeups_per_s",
            sum(procs.iter().map(|p| p.wakeups_per_s)),
            Unit::PerS,
        );
        m.lower(
            "pss_max",
            bytes(sum(procs.iter().map(|p| p.pss_max_bytes))),
            Unit::Bytes,
        );
        m.lower(
            "private_max",
            bytes(sum(procs.iter().map(|p| p.private_max_bytes))),
            Unit::Bytes,
        );
        m.lower(
            "gpu_proc_mem_max",
            bytes(sum(procs.iter().map(|p| p.gpu_mem_max_bytes))),
            Unit::Bytes,
        );
        m.lower(
            "gpu_proc_busy_percent",
            sum(procs.iter().map(|p| p.gpu_busy_percent)),
            Unit::Percent,
        );
        m.neutral("cpu_power_w", self.cpu_power_w, Unit::Watts);
        m.neutral("gpu_power_w", self.gpu_power_w, Unit::Watts);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        let n = self.processes.iter().map(|p| p.samples).sum::<usize>()
            + self.gpu.as_ref().map_or(0, |g| g.samples);
        (n > 0).then_some(n)
    }
}
