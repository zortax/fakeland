//! Resource sampling on the compositor thread: it starts the sampler thread, tells it about
//! launched and exited roots from the trace, records each batch as `ProcessSample` and
//! `GpuSample` events with compositor timestamps, and keeps the latest samples.

use std::collections::BTreeMap;

use fakeland_trace::Event;
use fakeland_types::{ProcessId, Timestamp};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::channel::{self, Channel};

use super::sampler::{Batch, Control, SamplerThread};
use super::{GpuReading, ProcessTreeSample, ResourceSnapshot};
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
        }
    }

    fn on_samples(&mut self, batch: Batch) {
        let (trees, gpu, ended) = match batch {
            Batch::GpuSource(name) => {
                if let Some(s) = &mut self.sampling {
                    s.gpu_source = name.map(str::to_string);
                }
                return;
            }
            Batch::Sample { trees, gpu, ended } => (trees, gpu, ended),
        };
        let now = self.now();
        let mut samples = Vec::new();
        for tree in trees {
            let Some(cpu) = tree.cpu_percent else {
                continue;
            };
            let command = self
                .processes
                .apps
                .get(&tree.root)
                .map(|a| a.command.clone())
                .unwrap_or_default();
            samples.push(ProcessTreeSample {
                pid: ProcessId(tree.root),
                command,
                time: now,
                cpu_percent: cpu,
                rss_bytes: tree.rss_bytes,
                threads: tree.threads,
                processes: tree.processes,
            });
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
    }
}
