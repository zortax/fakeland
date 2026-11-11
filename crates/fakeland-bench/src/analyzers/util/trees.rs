//! The process trees of an iteration: the trees under test, and the cumulative meters of each tree
//! and each energy domain between the first and the last sample inside the iteration.

use std::collections::{BTreeMap, BTreeSet};

use fakeland_trace::Event;
use fakeland_types::{ProcessId, Timestamp};

use crate::input::AnalysisInput;

/// The first and the last value of a cumulative counter inside the iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counter {
    /// The time and value of the first sample.
    pub first: (Timestamp, u64),
    /// The time and value of the last sample.
    pub last: (Timestamp, u64),
}

impl Counter {
    /// Adds a sample to an optional counter.
    pub fn push(counter: &mut Option<Self>, t: Timestamp, value: u64) {
        match counter {
            Some(c) => c.last = (t, value),
            None => {
                *counter = Some(Self {
                    first: (t, value),
                    last: (t, value),
                })
            }
        }
    }

    /// Returns the growth from the first to the last sample. A counter that restarts reads as 0.
    pub fn delta(&self) -> u64 {
        self.last.1.saturating_sub(self.first.1)
    }

    /// Returns the seconds from the first to the last sample.
    pub fn secs(&self) -> f64 {
        self.last.0.since(self.first.0).as_secs_f64()
    }

    /// Returns the growth per second, or `None` for a single sample.
    pub fn rate(&self) -> Option<f64> {
        let secs = self.secs();
        (secs > 0.0).then(|| self.delta() as f64 / secs)
    }
}

/// The cumulative meters of one tree inside the iteration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeMeters {
    /// The time on CPU in nanoseconds.
    pub cpu_ns: Option<Counter>,
    /// The context switches.
    pub switches: Option<Counter>,
    /// The largest proportional set size.
    pub pss_max: Option<u64>,
    /// The largest private memory.
    pub private_max: Option<u64>,
    /// The largest swapped memory.
    pub swap_max: Option<u64>,
    /// The largest peak resident set.
    pub hwm_max: Option<u64>,
    /// The largest GPU memory.
    pub gpu_mem_max: Option<u64>,
    /// The GPU busy time in nanoseconds.
    pub gpu_busy_ns: Option<Counter>,
    /// The GPU utilization samples in percent.
    pub gpu_util: Vec<f64>,
    /// The latest GPU source.
    pub gpu_source: Option<String>,
}

impl TreeMeters {
    /// Returns the GPU busy share in percent: the busy time over the wall time, else the mean
    /// utilization.
    pub fn gpu_busy_percent(&self) -> Option<f64> {
        self.gpu_busy_ns
            .and_then(|c| c.rate())
            .map(|ns_per_s| ns_per_s / 1e9 * 100.0)
            .or_else(|| {
                (!self.gpu_util.is_empty())
                    .then(|| self.gpu_util.iter().sum::<f64>() / self.gpu_util.len() as f64)
            })
    }
}

fn max_into(slot: &mut Option<u64>, v: Option<u64>) {
    if let Some(v) = v {
        *slot = Some(slot.map_or(v, |s| s.max(v)));
    }
}

/// Returns true when a record time lies inside the iteration.
fn inside(input: &AnalysisInput, t: Timestamp) -> bool {
    let (start, end) = input.span();
    t >= start && t <= end
}

/// Returns the meters of every tree with a detail or GPU sample inside the iteration.
pub fn tree_meters(input: &AnalysisInput) -> BTreeMap<ProcessId, TreeMeters> {
    let mut out: BTreeMap<ProcessId, TreeMeters> = BTreeMap::new();
    for r in input.trace.records.iter().filter(|r| inside(input, r.t)) {
        match &r.event {
            Event::ProcessDetail {
                pid,
                cpu_ns_total,
                ctx_switches_total,
                pss_bytes,
                private_bytes,
                swap_bytes,
                hwm_bytes,
            } => {
                let m = out.entry(*pid).or_default();
                Counter::push(&mut m.cpu_ns, r.t, *cpu_ns_total);
                Counter::push(&mut m.switches, r.t, *ctx_switches_total);
                max_into(&mut m.pss_max, *pss_bytes);
                max_into(&mut m.private_max, *private_bytes);
                max_into(&mut m.swap_max, *swap_bytes);
                max_into(&mut m.hwm_max, *hwm_bytes);
            }
            Event::GpuProcessSample {
                pid,
                mem_bytes,
                busy_ns_total,
                util_percent,
                source,
            } => {
                let m = out.entry(*pid).or_default();
                max_into(&mut m.gpu_mem_max, *mem_bytes);
                if let Some(b) = busy_ns_total {
                    Counter::push(&mut m.gpu_busy_ns, r.t, *b);
                }
                m.gpu_util.extend(util_percent);
                m.gpu_source = Some(source.clone());
            }
            _ => {}
        }
    }
    out
}

/// Returns the counter of every energy domain inside the iteration.
pub fn energy_counters(input: &AnalysisInput) -> BTreeMap<String, Counter> {
    let mut out: BTreeMap<String, Option<Counter>> = BTreeMap::new();
    for r in input.trace.records.iter().filter(|r| inside(input, r.t)) {
        if let Event::EnergySample {
            domain,
            microjoules_total,
        } = &r.event
        {
            Counter::push(
                out.entry(domain.clone()).or_default(),
                r.t,
                *microjoules_total,
            );
        }
    }
    out.into_iter().filter_map(|(d, c)| Some((d, c?))).collect()
}

/// Returns the processes that identify the target: the launched app, the client process, and
/// the client process of the target surface in the trace.
fn target_pids(input: &AnalysisInput) -> BTreeSet<ProcessId> {
    let Some(t) = &input.target else {
        return BTreeSet::new();
    };
    let tl = &input.timeline;
    let traced = tl
        .surface(t.surface)
        .and_then(|s| s.client)
        .and_then(|c| tl.client(c))
        .and_then(|c| c.pid);
    [t.app, t.pid, traced].into_iter().flatten().collect()
}

/// Returns the trees under test among `roots`: the trees whose root is a process of the target,
/// else every tree.
pub fn selected_roots(
    input: &AnalysisInput,
    roots: impl IntoIterator<Item = ProcessId>,
) -> BTreeSet<ProcessId> {
    let roots: BTreeSet<ProcessId> = roots.into_iter().collect();
    let wanted = target_pids(input);
    let matched: BTreeSet<ProcessId> = roots.intersection(&wanted).copied().collect();
    if matched.is_empty() { roots } else { matched }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Nanos;

    use super::*;
    use crate::testing::{Synth, at_ms};

    #[test]
    fn counters_and_selection() {
        let mut c = None;
        Counter::push(&mut c, at_ms(0.0), 100);
        assert_eq!(c.unwrap().rate(), None);
        Counter::push(&mut c, at_ms(500.0), 300);
        let c = c.unwrap();
        assert_eq!((c.delta(), c.rate()), (200, Some(400.0)));
        assert_eq!(c.first.0 + Nanos::from_millis(500), c.last.0);
        let s = Synth::new();
        let input = s.analysis();
        let roots = [ProcessId(100), ProcessId(200)];
        assert_eq!(
            selected_roots(&input, roots),
            BTreeSet::from([ProcessId(100)])
        );
        let other = s.analysis().with_target(None);
        assert_eq!(selected_roots(&other, roots).len(), 2);
        assert_eq!(selected_roots(&input, [ProcessId(200)]).len(), 1);
    }
}
