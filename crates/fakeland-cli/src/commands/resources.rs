//! The resource summary of `fakeland run`: CPU, memory and threads per app and GPU use from
//! the samples of a trace.

use fakeland::Event;
use fakeland_trace::Trace;
use fakeland_types::ProcessId;

/// Summarizes the resource samples of a trace: mean and peak CPU, peak memory and threads per
/// app, and GPU use.
pub fn resources_text(trace: &Trace) -> String {
    use std::collections::BTreeMap;
    let mut procs: BTreeMap<ProcessId, (Vec<f64>, u64, u32)> = BTreeMap::new();
    let mut commands: BTreeMap<ProcessId, String> = BTreeMap::new();
    let (mut gpu, mut gpu_mem) = (Vec::new(), 0u64);
    for r in &trace.records {
        match &r.event {
            Event::ProcessSpawned { pid, command } => {
                commands.insert(*pid, command.clone());
            }
            Event::ProcessSample {
                pid,
                cpu_percent,
                rss_bytes,
                threads,
            } => {
                let e = procs.entry(*pid).or_default();
                e.0.push(*cpu_percent);
                e.1 = e.1.max(*rss_bytes);
                e.2 = e.2.max(*threads);
            }
            Event::GpuSample {
                util_percent,
                mem_used_bytes,
                ..
            } => {
                gpu.push(*util_percent);
                gpu_mem = gpu_mem.max(*mem_used_bytes);
            }
            _ => {}
        }
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let peak = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
    let mut out = String::new();
    for (pid, (cpu, rss, threads)) in &procs {
        out.push_str(&format!(
            "resources: pid {} cpu mean {:.1}% peak {:.1}%, rss peak {:.1} MiB, threads {}, {} samples  {}\n",
            pid.0,
            mean(cpu),
            peak(cpu),
            *rss as f64 / 1048576.0,
            threads,
            cpu.len(),
            commands.get(pid).map(String::as_str).unwrap_or("")
        ));
    }
    if !gpu.is_empty() {
        out.push_str(&format!(
            "resources: gpu mean {:.1}% peak {:.1}%, memory peak {:.0} MiB, {} samples\n",
            mean(&gpu),
            peak(&gpu),
            gpu_mem as f64 / 1048576.0,
            gpu.len()
        ));
    }
    if out.is_empty() {
        out.push_str("resources: no samples\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::Event;
    use fakeland_types::ProcessId;

    #[test]
    fn summarizes_resource_samples() {
        use fakeland_trace::{Environment, Record, TraceHeader};
        use fakeland_types::Timestamp;
        let mut trace = Trace::new(TraceHeader::new(
            Environment::default(),
            Vec::new(),
            Timestamp(0),
        ));
        let mut push = |t: u64, event: Event| {
            trace.records.push(Record {
                t: Timestamp(t),
                event,
            })
        };
        push(
            1,
            Event::ProcessSpawned {
                pid: ProcessId(7),
                command: "vkcube".into(),
            },
        );
        for (t, cpu) in [(2, 10.0), (3, 30.0)] {
            push(
                t,
                Event::ProcessSample {
                    pid: ProcessId(7),
                    cpu_percent: cpu,
                    rss_bytes: 64 << 20,
                    threads: 5,
                },
            );
        }
        push(
            4,
            Event::GpuSample {
                util_percent: 40.0,
                mem_used_bytes: 1 << 30,
                mem_total_bytes: 12 << 30,
            },
        );
        let text = resources_text(&trace);
        assert!(text.contains("pid 7 cpu mean 20.0% peak 30.0%"), "{text}");
        assert!(
            text.contains("rss peak 64.0 MiB, threads 5, 2 samples  vkcube"),
            "{text}"
        );
        assert!(
            text.contains("gpu mean 40.0% peak 40.0%, memory peak 1024 MiB"),
            "{text}"
        );
        let empty = Trace::new(TraceHeader::new(
            Environment::default(),
            Vec::new(),
            Timestamp(0),
        ));
        assert_eq!(resources_text(&empty), "resources: no samples\n");
    }
}
