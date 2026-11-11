//! Resource sampling on the compositor thread: it starts the sampler thread, tells it about
//! launched and exited roots from the trace, records each batch as `ProcessSample`,
//! `ProcessDetail`, `GpuProcessSample`, `GpuSample` and `EnergySample` events with one compositor
//! timestamp, and keeps the latest samples.

use std::collections::BTreeMap;

use fakeland_trace::Event;
use fakeland_types::{ProcessId, Timestamp};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::channel::{self, Channel};

use super::collect::TreeReading;
use super::gpu::GpuProcessReading;
use super::sampler::{Batch, Control, SamplerThread};
use super::{EnergyReading, GpuReading, ProcessTreeSample, ResourceSnapshot};
use crate::config::SamplingConfig;
use crate::render::find_render_node;
use crate::server::State;
use crate::waits::{Observe, ObserverId};

/// The sampling state of a session.
#[derive(Debug)]
pub struct Sampling {
    config: SamplingConfig,
    thread: SamplerThread,
    source: RegistrationToken,
    observer: ObserverId,
    latest: BTreeMap<u32, ProcessTreeSample>,
    gpu: Option<GpuReading>,
    gpu_source: Option<String>,
    energy: Vec<EnergyReading>,
    time: Option<Timestamp>,
}

impl State {
    /// Starts resource sampling when the config asks for it.
    pub fn start_sampling(&mut self) -> std::io::Result<()> {
        let Some(config) = self.config.sampling else {
            return Ok(());
        };
        let node = match &self.render {
            crate::render::RenderBackend::Gles(d) => Some(d.path.clone()),
            crate::render::RenderBackend::Pixman(_) => {
                self.config.render_node.clone().or_else(find_render_node)
            }
        };
        let (tx, rx): (_, Channel<Batch>) = channel::channel();
        let source = self
            .loop_handle
            .insert_source(rx, |event, _, state| {
                if let channel::Event::Msg(batch) = event {
                    state.on_samples(batch);
                }
            })
            .map_err(|e| std::io::Error::other(e.error.to_string()))?;
        let thread = SamplerThread::spawn(config, self.clock.clone(), node, tx)?;
        let observer = self.observe(|state, record| {
            let Some(sampling) = &state.sampling else {
                return Observe::Remove;
            };
            match &record.event {
                Event::ProcessSpawned { pid, .. } => sampling.thread.send(Control::Add(pid.0)),
                Event::ProcessExited { pid, .. } => sampling.thread.send(Control::Exited(pid.0)),
                _ => {}
            }
            Observe::Keep
        });
        self.sampling = Some(Sampling {
            config,
            thread,
            source,
            observer,
            latest: BTreeMap::new(),
            gpu: None,
            gpu_source: None,
            energy: Vec::new(),
            time: None,
        });
        Ok(())
    }

    /// Stops the sampler thread.
    pub fn stop_sampling(&mut self) {
        if let Some(s) = self.sampling.take() {
            self.unobserve(s.observer);
            self.loop_handle.remove(s.source);
        }
    }

    /// Returns the latest samples.
    pub fn resource_snapshot(&self) -> ResourceSnapshot {
        let Some(s) = &self.sampling else {
            return ResourceSnapshot::disabled();
        };
        ResourceSnapshot {
            enabled: true,
            interval: s.config.interval,
            time: s.time,
            processes: s.latest.values().cloned().collect(),
            gpu: s.gpu,
            gpu_source: s.gpu_source.clone(),
            energy: s.energy.clone(),
        }
    }

    fn on_samples(&mut self, batch: Batch) {
        let Batch::Sample {
            trees,
            gpu,
            gpu_trees,
            energy,
            ended,
        } = batch
        else {
            if let (Batch::GpuSource(name), Some(s)) = (batch, &mut self.sampling) {
                s.gpu_source = name.map(str::to_string);
            }
            return;
        };
        let now = self.now();
        let mut samples = Vec::new();
        for tree in &trees {
            self.emit_at(now, detail_event(tree));
            let Some(cpu) = tree.cpu_percent else {
                continue;
            };
            let command = self
                .processes
                .apps
                .get(&tree.root)
                .map(|a| a.command.clone())
                .unwrap_or_default();
            let g = gpu_trees.iter().find(|g| g.root == tree.root);
            samples.push(tree_sample(tree, g, command, now, cpu));
        }
        for s in &samples {
            self.emit_at(
                now,
                Event::ProcessSample {
                    pid: s.pid,
                    cpu_percent: s.cpu_percent,
                    rss_bytes: s.rss_bytes,
                    threads: s.threads,
                },
            );
        }
        for g in &gpu_trees {
            self.emit_at(
                now,
                Event::GpuProcessSample {
                    pid: ProcessId(g.root),
                    mem_bytes: g.mem_bytes,
                    busy_ns_total: g.busy_ns_total,
                    util_percent: g.util_percent,
                    source: g.source.clone(),
                },
            );
        }
        if let Some(g) = gpu {
            self.emit_at(
                now,
                Event::GpuSample {
                    util_percent: g.util_percent,
                    mem_used_bytes: g.mem_used_bytes,
                    mem_total_bytes: g.mem_total_bytes,
                },
            );
        }
        for e in &energy {
            self.emit_at(
                now,
                Event::EnergySample {
                    domain: e.domain.clone(),
                    microjoules_total: e.microjoules_total,
                },
            );
        }
        let Some(state) = &mut self.sampling else {
            return;
        };
        state.time = Some(now);
        for root in ended {
            state.latest.remove(&root);
        }
        for s in samples {
            state.latest.insert(s.pid.0, s);
        }
        if gpu.is_some() {
            state.gpu = gpu;
        }
        if !energy.is_empty() {
            state.energy = energy;
        }
    }
}

/// Returns the detail event of a tree reading.
fn detail_event(tree: &TreeReading) -> Event {
    Event::ProcessDetail {
        pid: ProcessId(tree.root),
        cpu_ns_total: tree.cpu_ns_total,
        ctx_switches_total: tree.ctx_switches_total,
        pss_bytes: tree.memory.map(|m| m.pss_bytes),
        private_bytes: tree.memory.map(|m| m.private_bytes),
        swap_bytes: tree.memory.map(|m| m.swap_bytes),
        hwm_bytes: tree.hwm_bytes,
    }
}

/// Builds the snapshot entry of a tree.
fn tree_sample(
    tree: &TreeReading,
    gpu: Option<&GpuProcessReading>,
    command: String,
    time: Timestamp,
    cpu_percent: f64,
) -> ProcessTreeSample {
    ProcessTreeSample {
        pid: ProcessId(tree.root),
        command,
        time,
        cpu_percent,
        rss_bytes: tree.rss_bytes,
        threads: tree.threads,
        processes: tree.processes,
        cpu_ns_total: tree.cpu_ns_total,
        ctx_switches_total: tree.ctx_switches_total,
        pss_bytes: tree.memory.map(|m| m.pss_bytes),
        private_bytes: tree.memory.map(|m| m.private_bytes),
        swap_bytes: tree.memory.map(|m| m.swap_bytes),
        hwm_bytes: tree.hwm_bytes,
        gpu_mem_bytes: gpu.and_then(|g| g.mem_bytes),
        gpu_busy_ns_total: gpu.and_then(|g| g.busy_ns_total),
        gpu_util_percent: gpu.and_then(|g| g.util_percent),
        gpu_source: gpu.map(|g| g.source.clone()),
    }
}
