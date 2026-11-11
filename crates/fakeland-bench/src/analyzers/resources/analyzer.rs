//! The resources analyzer.

use std::collections::BTreeMap;

use fakeland_trace::{Event, Summary};
use fakeland_types::ProcessId;

use super::{EnergyDomain, GpuResources, ProcessResources, ResourcesReport};
use crate::analyzer::Analyzer;
use crate::analyzers::util::{TreeMeters, energy_counters, selected_roots, tree_meters};
use crate::input::AnalysisInput;

/// Summarizes the process, GPU and energy samples of the trace. Rates come from the cumulative
/// counters between the first and the last sample inside the iteration. Process metrics cover the
/// tree of the target when the trace has it, else every tree.
#[derive(Clone, Debug, Default)]
pub struct Resources;

#[derive(Default)]
struct Acc {
    cpu: Vec<f64>,
    rss: Vec<u64>,
    threads: u32,
}

impl Analyzer for Resources {
    type Report = ResourcesReport;

    fn name(&self) -> String {
        "resources".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> ResourcesReport {
        let mut procs: BTreeMap<ProcessId, Acc> = BTreeMap::new();
        let mut util = Vec::new();
        let mut mem_max = 0;
        let mut mem_total = 0;
        for r in &input.trace.records {
            match &r.event {
                Event::ProcessSample {
                    pid,
                    cpu_percent,
                    rss_bytes,
                    threads,
                } => {
                    let a = procs.entry(*pid).or_default();
                    a.cpu.push(*cpu_percent);
                    a.rss.push(*rss_bytes);
                    a.threads = a.threads.max(*threads);
                }
                Event::GpuSample {
                    util_percent,
                    mem_used_bytes,
                    mem_total_bytes,
                } => {
                    util.push(*util_percent);
                    mem_max = mem_max.max(*mem_used_bytes);
                    mem_total = *mem_total_bytes;
                }
                _ => {}
            }
        }
        let mut meters = tree_meters(input);
        for pid in meters.keys() {
            procs.entry(*pid).or_default();
        }
        let tl = &input.timeline;
        let processes: Vec<ProcessResources> = procs
            .into_iter()
            .map(|(pid, a)| {
                let m = meters.remove(&pid).unwrap_or_default();
                process(pid, tl.process(pid).map(|p| p.command.clone()), a, &m)
            })
            .collect();
        let selected = selected_roots(input, processes.iter().map(|p| p.pid))
            .into_iter()
            .collect();
        let energy: Vec<EnergyDomain> = energy_counters(input)
            .into_iter()
            .map(|(domain, c)| EnergyDomain {
                domain,
                joules: c.delta() as f64 / 1e6,
                watts: c.rate().map(|uj_per_s| uj_per_s / 1e6),
            })
            .collect();
        let power = |prefix: &str| {
            let w: Vec<f64> = energy
                .iter()
                .filter(|e| e.domain.starts_with(prefix))
                .filter_map(|e| e.watts)
                .collect();
            (!w.is_empty()).then(|| w.iter().sum())
        };
        ResourcesReport {
            cpu_power_w: power("cpu-"),
            gpu_power_w: power("gpu-"),
            processes,
            gpu: (!util.is_empty()).then(|| GpuResources {
                samples: util.len(),
                util_percent: Summary::of(&util),
                mem_used_max_bytes: mem_max,
                mem_total_bytes: mem_total,
            }),
            selected,
            energy,
        }
    }
}

fn process(pid: ProcessId, command: Option<String>, a: Acc, m: &TreeMeters) -> ProcessResources {
    ProcessResources {
        pid,
        command,
        samples: a.cpu.len(),
        cpu_percent: Summary::of(&a.cpu),
        rss_max_bytes: a.rss.iter().copied().max().unwrap_or(0),
        rss_mean_bytes: if a.rss.is_empty() {
            0.0
        } else {
            a.rss.iter().map(|v| *v as f64).sum::<f64>() / a.rss.len() as f64
        },
        threads_max: a.threads,
        cpu_ms: m
            .cpu_ns
            .filter(|c| c.secs() > 0.0)
            .map(|c| c.delta() as f64 / 1e6),
        cpu_ms_per_s: m
            .cpu_ns
            .and_then(|c| c.rate())
            .map(|ns_per_s| ns_per_s / 1e6),
        wakeups_per_s: m.switches.and_then(|c| c.rate()),
        pss_max_bytes: m.pss_max,
        private_max_bytes: m.private_max,
        swap_max_bytes: m.swap_max,
        hwm_max_bytes: m.hwm_max,
        gpu_mem_max_bytes: m.gpu_mem_max,
        gpu_busy_percent: m.gpu_busy_percent(),
        gpu_source: m.gpu_source.clone(),
    }
}
