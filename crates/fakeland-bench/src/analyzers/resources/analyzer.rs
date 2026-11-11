//! The resources analyzer.

use std::collections::BTreeMap;

use fakeland_trace::{Event, Summary};
use fakeland_types::ProcessId;

use super::{GpuResources, ProcessResources, ResourcesReport};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Summarizes the process and GPU samples of the trace.
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
        let tl = &input.timeline;
        ResourcesReport {
            processes: procs
                .into_iter()
                .map(|(pid, a)| ProcessResources {
                    pid,
                    command: tl.process(pid).map(|p| p.command.clone()),
                    samples: a.cpu.len(),
                    cpu_percent: Summary::of(&a.cpu),
                    rss_max_bytes: a.rss.iter().copied().max().unwrap_or(0),
                    rss_mean_bytes: if a.rss.is_empty() {
                        0.0
                    } else {
                        a.rss.iter().map(|v| *v as f64).sum::<f64>() / a.rss.len() as f64
                    },
                    threads_max: a.threads,
                })
                .collect(),
            gpu: (!util.is_empty()).then(|| GpuResources {
                samples: util.len(),
                util_percent: Summary::of(&util),
                mem_used_max_bytes: mem_max,
                mem_total_bytes: mem_total,
            }),
        }
    }
}
