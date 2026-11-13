//! The sampler thread. It wakes at each sample time on the session clock, reads `/proc` for the
//! trees of the launched roots, reads the GPU probes and the energy counters, then hands one batch
//! to the compositor thread, which records it. Reads never run on the compositor thread. The
//! reads that stall other GPU work run once per slow interval, and hold the GPU gate.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};

use fakeland_types::{Nanos, SharedClock};
use smithay::reexports::calloop::channel::Sender;

use super::collect::{TreeReading, TreeSample, Trees};
use super::energy::{EnergyMeter, EnergyReading};
use super::gate::GpuGate;
use super::gpu::{
    GpuContext, GpuProcessChain, GpuProcessReading, GpuReading, TreeMembers, open_probe,
};
use crate::config::SamplingConfig;

/// A control message to the sampler thread.
#[derive(Debug, Clone, Copy)]
pub enum Control {
    /// Sample the tree of a launched root.
    Add(u32),
    /// The root ended. Its tree drops once no member lives.
    Exited(u32),
    /// End the thread.
    Stop,
}

/// One sample of every source.
#[derive(Debug, Clone)]
pub enum Batch {
    /// The GPU probe opened, or no probe works.
    GpuSource(Option<&'static str>),
    /// One sample.
    Sample {
        /// The trees.
        trees: Vec<TreeReading>,
        /// The GPU reading.
        gpu: Option<GpuReading>,
        /// The GPU readings of the trees.
        gpu_trees: Vec<GpuProcessReading>,
        /// The energy counters.
        energy: Vec<EnergyReading>,
        /// Roots whose trees ended.
        ended: Vec<u32>,
    },
}

/// The handle of the sampler thread. Dropping it stops the thread.
#[derive(Debug)]
pub struct SamplerThread {
    control: mpsc::Sender<Control>,
    thread: Option<JoinHandle<()>>,
}

impl SamplerThread {
    /// Starts the thread.
    pub fn spawn(
        config: SamplingConfig,
        clock: SharedClock,
        node: Option<PathBuf>,
        gate: GpuGate,
        out: Sender<Batch>,
    ) -> std::io::Result<Self> {
        let (control, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("fakeland-sampler".into())
            .spawn(move || run(config, clock, node, gate, rx, out))?;
        Ok(Self {
            control,
            thread: Some(thread),
        })
    }

    /// Sends a control message.
    pub fn send(&self, msg: Control) {
        let _ = self.control.send(msg);
    }
}

impl Drop for SamplerThread {
    fn drop(&mut self) {
        let _ = self.control.send(Control::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    config: SamplingConfig,
    clock: SharedClock,
    node: Option<PathBuf>,
    gate: GpuGate,
    rx: mpsc::Receiver<Control>,
    out: Sender<Batch>,
) {
    let ctx = GpuContext::open(
        node.as_deref(),
        config.gpu || config.gpu_process || config.energy,
    );
    let mut gpu = config.gpu.then(|| open_probe(&ctx)).flatten();
    if config.gpu
        && out
            .send(Batch::GpuSource(gpu.as_ref().map(|p| p.name())))
            .is_err()
    {
        return;
    }
    let mut gpu_trees = (config.process && config.gpu_process).then(|| GpuProcessChain::open(&ctx));
    if let Some(chain) = &gpu_trees {
        tracing::debug!("per-tree GPU backends: {:?}", chain.names());
    }
    let mut energy = config.energy.then(|| EnergyMeter::open(&ctx));
    if let Some(meter) = &energy {
        tracing::debug!("energy domains: {:?}", meter.names());
    }
    let mut trees = Trees::default();
    let mut next = clock.now();
    let mut next_slow = next;
    loop {
        let wait = next.since(clock.now()).max(Nanos::ZERO);
        match rx.recv_timeout(wait.to_duration()) {
            Ok(Control::Add(pid)) => {
                trees.roots.insert(pid);
                trees.exited.remove(&pid);
                continue;
            }
            Ok(Control::Exited(pid)) => {
                trees.exited.insert(pid);
                continue;
            }
            Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let t = clock.now();
        let sample = if config.process && !trees.roots.is_empty() {
            trees.sample(t, config.memory_detail)
        } else {
            TreeSample::default()
        };
        let slow = t >= next_slow;
        if slow {
            next_slow = t + config.slow_interval;
        }
        let gpu_reading = gpu.as_mut().and_then(|p| p.read());
        // The slow reads hold the gate, so no readback of the compositor thread runs into them.
        let held = slow.then(|| gate.enter());
        let gpu_tree_readings = match &mut gpu_trees {
            Some(chain) => {
                for root in &sample.ended {
                    chain.forget(*root);
                }
                let members: Vec<TreeMembers<'_>> = sample
                    .members
                    .iter()
                    .map(|(root, pids)| TreeMembers { root: *root, pids })
                    .collect();
                chain.read(t, &members, slow)
            }
            None => Vec::new(),
        };
        let energy_readings = energy
            .as_mut()
            .map(|meter| meter.read(slow))
            .unwrap_or_default();
        drop(held);
        let batch = Batch::Sample {
            trees: sample.readings,
            gpu: gpu_reading,
            gpu_trees: gpu_tree_readings,
            energy: energy_readings,
            ended: sample.ended,
        };
        if out.send(batch).is_err() {
            return;
        }
        let now = clock.now();
        next += config.interval;
        if next < now {
            next = now + config.interval;
        }
    }
}
