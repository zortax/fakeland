//! The latest resource samples of a session, as the session API returns them.

use fakeland_types::dur::{Dur, as_dur};
use fakeland_types::{Nanos, ProcessId, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{EnergyReading, GpuReading};

/// The resources of one launched process tree at one sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProcessTreeSample {
    /// The launched root process.
    pub pid: ProcessId,
    /// The command line of the root.
    pub command: String,
    /// The sample time.
    pub time: Timestamp,
    /// CPU use since the previous sample in percent of one core.
    pub cpu_percent: f64,
    /// Resident memory of every member in bytes.
    pub rss_bytes: u64,
    /// Threads of every member.
    pub threads: u32,
    /// The member processes.
    pub processes: u32,
    /// The time on CPU of every member since the first sample of the tree, in nanoseconds.
    #[serde(default)]
    pub cpu_ns_total: u64,
    /// The context switches of every member thread since the first sample of the tree.
    #[serde(default)]
    pub ctx_switches_total: u64,
    /// The proportional set size of every member in bytes.
    #[serde(default)]
    pub pss_bytes: Option<u64>,
    /// The private memory of every member in bytes.
    #[serde(default)]
    pub private_bytes: Option<u64>,
    /// The swapped memory of every member in bytes.
    #[serde(default)]
    pub swap_bytes: Option<u64>,
    /// The largest peak resident set of a member in bytes.
    #[serde(default)]
    pub hwm_bytes: Option<u64>,
    /// GPU memory of every member in bytes.
    #[serde(default)]
    pub gpu_mem_bytes: Option<u64>,
    /// The GPU engine busy time of every member since the first sample, in nanoseconds.
    #[serde(default)]
    pub gpu_busy_ns_total: Option<u64>,
    /// GPU utilization of every member in percent.
    #[serde(default)]
    pub gpu_util_percent: Option<f64>,
    /// The source of the GPU values of the tree, for example `nvml` or `fdinfo:amdgpu`.
    #[serde(default)]
    pub gpu_source: Option<String>,
}

/// The latest samples of every source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceSnapshot {
    /// True when the session samples resources.
    pub enabled: bool,
    /// The sample interval.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub interval: Nanos,
    /// The time of the latest sample.
    pub time: Option<Timestamp>,
    /// The latest sample of each live process tree, by pid.
    pub processes: Vec<ProcessTreeSample>,
    /// The latest GPU reading.
    pub gpu: Option<GpuReading>,
    /// The GPU source, for example `nvml` or `sysfs`. `None` means no GPU samples.
    pub gpu_source: Option<String>,
    /// The latest reading of each energy domain.
    #[serde(default)]
    pub energy: Vec<EnergyReading>,
}

impl ResourceSnapshot {
    /// Returns the snapshot of a session without sampling.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            interval: Nanos::ZERO,
            time: None,
            processes: Vec::new(),
            gpu: None,
            gpu_source: None,
            energy: Vec::new(),
        }
    }

    /// Returns a short text, one line per process tree and one for the GPU.
    pub fn to_text(&self) -> String {
        if !self.enabled {
            return "resource sampling is off".into();
        }
        let mut out = String::new();
        if self.processes.is_empty() {
            out.push_str("no process samples yet\n");
        }
        for p in &self.processes {
            out.push_str(&format!(
                "pid {} {:>6.1}% cpu {:>8.1} MiB rss {:>4} threads {:>3} procs  {}\n",
                p.pid.0,
                p.cpu_percent,
                p.rss_bytes as f64 / MIB,
                p.threads,
                p.processes,
                p.command
            ));
            out.push_str(&detail_line(p));
        }
        match (&self.gpu, &self.gpu_source) {
            (Some(g), Some(src)) => out.push_str(&format!(
                "gpu {:>5.1}% util {:.0}/{:.0} MiB ({src})\n",
                g.util_percent,
                g.mem_used_bytes as f64 / MIB,
                g.mem_total_bytes as f64 / MIB
            )),
            _ => out.push_str("no gpu samples\n"),
        }
        for e in &self.energy {
            out.push_str(&format!(
                "energy {} {:.3} J\n",
                e.domain,
                e.microjoules_total as f64 / 1e6
            ));
        }
        out
    }
}

const MIB: f64 = 1024.0 * 1024.0;

/// Returns the line of cumulative counters, memory detail and GPU use of a tree.
fn detail_line(p: &ProcessTreeSample) -> String {
    let mib =
        |v: Option<u64>| v.map_or_else(|| "-".to_string(), |b| format!("{:.1}", b as f64 / MIB));
    let mut line = format!(
        "    cpu {:.1} ms total, {} switches, pss {} MiB, private {} MiB, swap {} MiB, hwm {} MiB",
        p.cpu_ns_total as f64 / 1e6,
        p.ctx_switches_total,
        mib(p.pss_bytes),
        mib(p.private_bytes),
        mib(p.swap_bytes),
        mib(p.hwm_bytes),
    );
    if let Some(src) = &p.gpu_source {
        line.push_str(&format!(", gpu mem {} MiB", mib(p.gpu_mem_bytes)));
        if let Some(ns) = p.gpu_busy_ns_total {
            line.push_str(&format!(", gpu busy {:.1} ms total", ns as f64 / 1e6));
        }
        if let Some(u) = p.gpu_util_percent {
            line.push_str(&format!(", gpu {u:.1}%"));
        }
        line.push_str(&format!(" ({src})"));
    }
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lists_trees_and_gpu() {
        let snap = ResourceSnapshot {
            enabled: true,
            interval: Nanos::from_millis(100),
            time: Some(Timestamp(5)),
            processes: vec![ProcessTreeSample {
                pid: ProcessId(42),
                command: "vkcube".into(),
                time: Timestamp(5),
                cpu_percent: 12.5,
                rss_bytes: 64 << 20,
                threads: 9,
                processes: 1,
                cpu_ns_total: 1_500_000_000,
                ctx_switches_total: 4200,
                pss_bytes: Some(48 << 20),
                private_bytes: Some(40 << 20),
                swap_bytes: Some(0),
                hwm_bytes: Some(70 << 20),
                gpu_mem_bytes: Some(256 << 20),
                gpu_busy_ns_total: None,
                gpu_util_percent: Some(12.0),
                gpu_source: Some("nvml".into()),
            }],
            gpu: Some(GpuReading {
                util_percent: 30.0,
                mem_used_bytes: 1 << 30,
                mem_total_bytes: 12 << 30,
            }),
            gpu_source: Some("nvml".into()),
            energy: vec![EnergyReading {
                domain: "gpu-nvml-0".into(),
                microjoules_total: 2_500_000,
            }],
        };
        let text = snap.to_text();
        assert!(text.contains("pid 42"), "{text}");
        assert!(text.contains("12.5% cpu"), "{text}");
        assert!(text.contains("64.0 MiB rss"), "{text}");
        assert!(text.contains("1024/12288 MiB (nvml)"), "{text}");
        assert!(
            text.contains("cpu 1500.0 ms total, 4200 switches"),
            "{text}"
        );
        assert!(text.contains("pss 48.0 MiB, private 40.0 MiB"), "{text}");
        assert!(
            text.contains("gpu mem 256.0 MiB, gpu 12.0% (nvml)"),
            "{text}"
        );
        assert!(text.contains("energy gpu-nvml-0 2.500 J"), "{text}");
        let back: ResourceSnapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back, snap);
        assert!(ResourceSnapshot::disabled().to_text().contains("off"));
    }
}
