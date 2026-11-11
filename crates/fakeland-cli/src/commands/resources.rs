//! The resource summary of `fakeland run`: CPU, memory and threads per app, CPU time, wakeups,
//! memory detail and GPU use per app, device GPU use and energy, from the samples of a trace.

use fakeland::Event;
use fakeland_trace::Trace;
use fakeland_types::{ProcessId, Timestamp};

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
    out.push_str(&meters_text(trace));
    if out.is_empty() {
        out.push_str("resources: no samples\n");
    }
    out
}

/// The first and last value of a cumulative counter.
#[derive(Clone, Copy)]
struct Span {
    first: (Timestamp, u64),
    last: (Timestamp, u64),
}

impl Span {
    fn push(span: &mut Option<Self>, t: Timestamp, v: u64) {
        match span {
            Some(s) => s.last = (t, v),
            None => {
                *span = Some(Self {
                    first: (t, v),
                    last: (t, v),
                })
            }
        }
    }

    /// Returns the growth per second.
    fn rate(self) -> Option<f64> {
        let secs = self.last.0.since(self.first.0).as_secs_f64();
        (secs > 0.0).then(|| self.last.1.saturating_sub(self.first.1) as f64 / secs)
    }
}

#[derive(Default)]
struct Meters {
    cpu: Option<Span>,
    switches: Option<Span>,
    pss: Option<u64>,
    private: Option<u64>,
    gpu_mem: Option<u64>,
    gpu_busy: Option<Span>,
    gpu_util: Vec<f64>,
    gpu_source: Option<String>,
}

/// Summarizes the cumulative meters: CPU time and wakeups per second, peak memory detail and GPU
/// use per app, and the power of each energy domain.
fn meters_text(trace: &Trace) -> String {
    use std::collections::BTreeMap;
    let mut procs: BTreeMap<ProcessId, Meters> = BTreeMap::new();
    let mut energy: BTreeMap<String, Option<Span>> = BTreeMap::new();
    let max = |a: &mut Option<u64>, b: Option<u64>| {
        if let Some(b) = b {
            *a = Some(a.unwrap_or(0).max(b));
        }
    };
    for r in &trace.records {
        match &r.event {
            Event::ProcessDetail {
                pid,
                cpu_ns_total,
                ctx_switches_total,
                pss_bytes,
                private_bytes,
                ..
            } => {
                let m = procs.entry(*pid).or_default();
                Span::push(&mut m.cpu, r.t, *cpu_ns_total);
                Span::push(&mut m.switches, r.t, *ctx_switches_total);
                max(&mut m.pss, *pss_bytes);
                max(&mut m.private, *private_bytes);
            }
            Event::GpuProcessSample {
                pid,
                mem_bytes,
                busy_ns_total,
                util_percent,
                source,
            } => {
                let m = procs.entry(*pid).or_default();
                max(&mut m.gpu_mem, *mem_bytes);
                if let Some(b) = busy_ns_total {
                    Span::push(&mut m.gpu_busy, r.t, *b);
                }
                m.gpu_util.extend(util_percent);
                m.gpu_source = Some(source.clone());
            }
            Event::EnergySample {
                domain,
                microjoules_total,
            } => Span::push(
                energy.entry(domain.clone()).or_default(),
                r.t,
                *microjoules_total,
            ),
            _ => {}
        }
    }
    let mib = |v: Option<u64>| {
        v.map_or_else(
            || "-".to_string(),
            |b| format!("{:.1}", b as f64 / 1048576.0),
        )
    };
    let mut out = String::new();
    for (pid, m) in &procs {
        let rate = |s: Option<Span>, scale: f64| {
            s.and_then(Span::rate)
                .map_or_else(|| "-".to_string(), |v| format!("{:.1}", v / scale))
        };
        out.push_str(&format!(
            "resources: pid {} cpu {} ms/s, wakeups {}/s, pss peak {} MiB, private peak {} MiB",
            pid.0,
            rate(m.cpu, 1e6),
            rate(m.switches, 1.0),
            mib(m.pss),
            mib(m.private),
        ));
        if let Some(src) = &m.gpu_source {
            let busy = m
                .gpu_busy
                .and_then(Span::rate)
                .map(|ns| ns / 1e7)
                .or_else(|| {
                    (!m.gpu_util.is_empty())
                        .then(|| m.gpu_util.iter().sum::<f64>() / m.gpu_util.len() as f64)
                });
            out.push_str(&format!(", gpu mem peak {} MiB", mib(m.gpu_mem)));
            if let Some(b) = busy {
                out.push_str(&format!(", gpu busy {b:.1}%"));
            }
            out.push_str(&format!(" ({src})"));
        }
        out.push('\n');
    }
    for (domain, span) in &energy {
        if let Some(w) = span.and_then(Span::rate) {
            out.push_str(&format!("resources: energy {domain} {:.2} W\n", w / 1e6));
        }
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
        for (t, cpu, uj) in [(2, 0, 0), (1_000_000_002, 250_000_000, 15_000_000)] {
            push(
                t,
                Event::ProcessDetail {
                    pid: ProcessId(7),
                    cpu_ns_total: cpu,
                    ctx_switches_total: cpu / 1_000_000,
                    pss_bytes: Some(32 << 20),
                    private_bytes: None,
                    swap_bytes: None,
                    hwm_bytes: None,
                },
            );
            push(
                t,
                Event::GpuProcessSample {
                    pid: ProcessId(7),
                    mem_bytes: Some(100 << 20),
                    busy_ns_total: None,
                    util_percent: Some(20.0),
                    source: "nvml".into(),
                },
            );
            push(
                t,
                Event::EnergySample {
                    domain: "gpu-nvml-0".into(),
                    microjoules_total: uj,
                },
            );
        }
        trace.records.sort_by_key(|r| r.t);
        let text = resources_text(&trace);
        assert!(text.contains("pid 7 cpu mean 20.0% peak 30.0%"), "{text}");
        assert!(
            text.contains("pid 7 cpu 250.0 ms/s, wakeups 250.0/s, pss peak 32.0 MiB, private peak - MiB, gpu mem peak 100.0 MiB, gpu busy 20.0% (nvml)"),
            "{text}"
        );
        assert!(text.contains("energy gpu-nvml-0 15.00 W"), "{text}");
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
