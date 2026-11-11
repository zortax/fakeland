//! The resources report.

use fakeland_trace::Summary;
use fakeland_types::ProcessId;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The samples of one process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessResources {
    /// The process.
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

/// Resource use over an iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourcesReport {
    /// Every sampled process, by pid.
    pub processes: Vec<ProcessResources>,
    /// The GPU, when the trace has GPU samples.
    pub gpu: Option<GpuResources>,
}

impl Metrics for ResourcesReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        if !self.processes.is_empty() {
            let cpu: f64 = self
                .processes
                .iter()
                .filter_map(|p| p.cpu_percent.mean())
                .sum();
            let rss: u64 = self.processes.iter().map(|p| p.rss_max_bytes).sum();
            let threads: u32 = self.processes.iter().map(|p| p.threads_max).sum();
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
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        let n = self.processes.iter().map(|p| p.samples).sum::<usize>()
            + self.gpu.as_ref().map_or(0, |g| g.samples);
        (n > 0).then_some(n)
    }
}
