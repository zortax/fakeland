//! Counter tracks of the cumulative meters: CPU time, wakeups and memory of each process tree,
//! GPU memory and busy share of each tree, and the power of each energy domain. A rate counter
//! takes the delta of a cumulative value since the previous sample of the same key.

use std::collections::HashMap;

use fakeland_types::{ProcessId, Timestamp};

use crate::event::Event;
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::{COMPOSITOR_PID, Emitter};

const MIB: f64 = 1024.0 * 1024.0;

/// The previous value of each cumulative counter.
#[derive(Default)]
struct Previous {
    values: HashMap<(String, u64), (Timestamp, u64)>,
}

impl Previous {
    /// Returns the rate per second of a counter since its previous value, and stores the value.
    fn rate(&mut self, name: &str, key: u64, t: Timestamp, value: u64) -> Option<f64> {
        let prev = self.values.insert((name.to_string(), key), (t, value))?;
        let secs = t.since(prev.0).as_secs_f64();
        (secs > 0.0 && value >= prev.1).then(|| (value - prev.1) as f64 / secs)
    }
}

/// Emits the meter counters.
pub(crate) fn emit(em: &mut Emitter<'_>) {
    let trace = em.trace;
    let mut prev = Previous::default();
    for r in &trace.records {
        let ts = em.us(r.t);
        match &r.event {
            Event::ProcessDetail {
                pid,
                cpu_ns_total,
                ctx_switches_total,
                pss_bytes,
                private_bytes,
                swap_bytes,
                ..
            } => {
                let p = em.os_pid(*pid);
                let key = u64::from(pid.0);
                if let Some(v) = prev.rate("cpu", key, r.t, *cpu_ns_total) {
                    em.push(ChromeEvent::counter("cpu ms/s", ts, p, v / 1e6));
                }
                if let Some(v) = prev.rate("wakeups", key, r.t, *ctx_switches_total) {
                    em.push(ChromeEvent::counter("wakeups/s", ts, p, v));
                }
                for (name, v) in [
                    ("pss MiB", pss_bytes),
                    ("private MiB", private_bytes),
                    ("swap MiB", swap_bytes),
                ] {
                    if let Some(v) = v {
                        em.push(ChromeEvent::counter(name, ts, p, *v as f64 / MIB));
                    }
                }
            }
            Event::GpuProcessSample {
                pid,
                mem_bytes,
                busy_ns_total,
                util_percent,
                ..
            } => gpu_process(
                em,
                &mut prev,
                (*pid, r.t),
                *mem_bytes,
                *busy_ns_total,
                *util_percent,
            ),
            Event::EnergySample {
                domain,
                microjoules_total,
            } => {
                if let Some(v) = prev.rate(domain, 0, r.t, *microjoules_total) {
                    em.push(ChromeEvent::counter(
                        format!("{domain} W"),
                        ts,
                        COMPOSITOR_PID,
                        v / 1e6,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn gpu_process(
    em: &mut Emitter<'_>,
    prev: &mut Previous,
    (pid, t): (ProcessId, Timestamp),
    mem: Option<u64>,
    busy_ns: Option<u64>,
    util: Option<f64>,
) {
    let ts = em.us(t);
    let p = em.os_pid(pid);
    if let Some(m) = mem {
        em.push(ChromeEvent::counter(
            "gpu proc mem MiB",
            ts,
            p,
            m as f64 / MIB,
        ));
    }
    let busy = busy_ns
        .and_then(|b| prev.rate("gpu busy", u64::from(pid.0), t, b))
        .map(|ns_per_s| ns_per_s / 1e9 * 100.0)
        .or(util);
    if let Some(v) = busy {
        em.push(ChromeEvent::counter("gpu proc %", ts, p, v));
    }
}
