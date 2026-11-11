//! The latest resource samples of a session, as the session API returns them.

use fakeland_types::dur::{Dur, as_dur};
use fakeland_types::{Nanos, ProcessId, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::GpuReading;

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
                p.rss_bytes as f64 / (1024.0 * 1024.0),
                p.threads,
                p.processes,
                p.command
            ));
        }
        match (&self.gpu, &self.gpu_source) {
            (Some(g), Some(src)) => out.push_str(&format!(
                "gpu {:>5.1}% util {:.0}/{:.0} MiB ({src})\n",
                g.util_percent,
                g.mem_used_bytes as f64 / (1024.0 * 1024.0),
                g.mem_total_bytes as f64 / (1024.0 * 1024.0)
            )),
            _ => out.push_str("no gpu samples\n"),
        }
        out
    }
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
            }],
            gpu: Some(GpuReading {
                util_percent: 30.0,
                mem_used_bytes: 1 << 30,
                mem_total_bytes: 12 << 30,
            }),
            gpu_source: Some("nvml".into()),
        };
        let text = snap.to_text();
        assert!(text.contains("pid 42"), "{text}");
        assert!(text.contains("12.5% cpu"), "{text}");
        assert!(text.contains("64.0 MiB rss"), "{text}");
        assert!(text.contains("1024/12288 MiB (nvml)"), "{text}");
        let back: ResourceSnapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back, snap);
        assert!(ResourceSnapshot::disabled().to_text().contains("off"));
    }
}
