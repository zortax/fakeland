//! The sampler thread. It wakes at each sample time on the session clock, reads `/proc` for the
//! trees of the launched roots and reads the GPU probe, then hands one batch to the compositor
//! thread, which records it. Reads never run on the compositor thread.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};

use fakeland_types::{Nanos, SharedClock, Timestamp};
use smithay::reexports::calloop::channel::Sender;

use super::cpu::{CpuAccount, Member};
use super::gpu::{GpuReading, open_probe};
use super::procfs::{StatLine, read_all_stats, read_status, read_thread_runtimes, read_uptime};
use super::tree::members;
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

/// The reading of one tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeReading {
    /// The root pid.
    pub root: u32,
    /// CPU use in percent of one core, `None` on the first sample of the tree.
    pub cpu_percent: Option<f64>,
    /// Resident memory in bytes.
    pub rss_bytes: u64,
    /// Threads.
    pub threads: u32,
    /// Member processes.
    pub processes: u32,
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
        out: Sender<Batch>,
    ) -> std::io::Result<Self> {
        let (control, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("fakeland-sampler".into())
            .spawn(move || run(config, clock, node, rx, out))?;
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

struct Trees {
    roots: BTreeSet<u32>,
    exited: BTreeSet<u32>,
    cpu: CpuAccount,
}

fn run(
    config: SamplingConfig,
    clock: SharedClock,
    node: Option<PathBuf>,
    rx: mpsc::Receiver<Control>,
    out: Sender<Batch>,
) {
    let mut gpu = config.gpu.then(|| open_probe(node.as_deref())).flatten();
    if config.gpu
        && out
            .send(Batch::GpuSource(gpu.as_ref().map(|p| p.name())))
            .is_err()
    {
        return;
    }
    let mut trees = Trees {
        roots: BTreeSet::new(),
        exited: BTreeSet::new(),
        cpu: CpuAccount::default(),
    };
    let mut next = clock.now();
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
        let (readings, ended) = if config.process && !trees.roots.is_empty() {
            sample_trees(&mut trees, clock.now())
        } else {
            (Vec::new(), Vec::new())
        };
        let gpu_reading = gpu.as_mut().and_then(|p| p.read());
        let batch = Batch::Sample {
            trees: readings,
            gpu: gpu_reading,
            ended,
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

fn sample_trees(trees: &mut Trees, t: Timestamp) -> (Vec<TreeReading>, Vec<u32>) {
    let stats = read_all_stats();
    let uptime = read_uptime().unwrap_or(0);
    let by_root = members(&stats, &trees.roots);
    let stat_of: BTreeMap<u32, &StatLine> = stats.iter().map(|s| (s.pid, s)).collect();
    let mut readings = Vec::new();
    let mut ended = Vec::new();
    for (root, pids) in by_root {
        if pids.is_empty() {
            if trees.exited.contains(&root) {
                ended.push(root);
            }
            continue;
        }
        let mut list = Vec::with_capacity(pids.len());
        let (mut rss, mut threads) = (0u64, 0u32);
        for pid in &pids {
            let Some(stat) = stat_of.get(pid) else {
                continue;
            };
            list.push(Member {
                pid: *pid,
                starttime: stat.starttime,
                ticks: stat.ticks(),
                threads: read_thread_runtimes(*pid),
            });
            if let Some(status) = read_status(*pid) {
                rss += status.rss_bytes;
                threads += status.threads;
            }
        }
        readings.push(TreeReading {
            root,
            cpu_percent: trees.cpu.update(root, t, uptime, &list),
            rss_bytes: rss,
            threads,
            processes: list.len() as u32,
        });
    }
    for root in &ended {
        trees.roots.remove(root);
        trees.exited.remove(root);
        trees.cpu.forget(*root);
    }
    (readings, ended)
}
