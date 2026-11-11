//! The launched processes of a session: their shared state, launch outputs, ancestry lookups and
//! teardown of every process group.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use fakeland_types::{OutputId, ProcessId};
use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::rustix::process::Signal;

use super::{A11yBus, AppShared, DbusDaemon, ExitInfo, proctree};

/// A child exit reported by its waiter thread.
#[derive(Debug, Clone, Copy)]
pub struct ExitMsg {
    /// The pid.
    pub pid: ProcessId,
    /// How it ended.
    pub info: ExitInfo,
}

/// Every launched process.
#[derive(Debug)]
pub struct Processes {
    /// Launched processes by pid.
    pub apps: BTreeMap<u32, Arc<AppShared>>,
    /// The launch output of each process.
    pub outputs: HashMap<u32, OutputId>,
    /// The channel for exit reports.
    pub exits: Sender<ExitMsg>,
    /// The accessibility bus of the private bus. It drops before the private bus.
    pub a11y: Option<A11yBus>,
    /// The private bus.
    pub dbus: Option<DbusDaemon>,
    /// Bytes of stdout and stderr each child keeps.
    pub log_capacity: usize,
}

impl Processes {
    /// Builds an empty set.
    pub fn new(exits: Sender<ExitMsg>, dbus: Option<DbusDaemon>, log_capacity: usize) -> Self {
        Self {
            apps: BTreeMap::new(),
            outputs: HashMap::new(),
            exits,
            a11y: None,
            dbus,
            log_capacity,
        }
    }

    /// Returns the launched process that `pid` descends from.
    pub fn root_of(&self, pid: ProcessId) -> Option<ProcessId> {
        let roots: BTreeSet<u32> = self.apps.keys().copied().collect();
        proctree::find_root(pid.0, &roots, proctree::read_stat).map(ProcessId)
    }

    /// Returns the launch output of the process tree of `pid`.
    pub fn output_for(&self, pid: ProcessId) -> Option<OutputId> {
        let root = self.root_of(pid)?;
        self.outputs.get(&root.0).copied()
    }

    /// Sends SIGTERM to every process group with a member, also when its leader has exited,
    /// waits up to `grace` for the groups to empty, then sends SIGKILL to the rest.
    pub fn kill_all(&mut self, grace: Duration) {
        let live: Vec<Arc<AppShared>> = self
            .apps
            .values()
            .filter(|a| a.group_alive())
            .cloned()
            .collect();
        for app in &live {
            let _ = app.signal(Signal::TERM);
        }
        let step = Duration::from_millis(10);
        let mut waited = Duration::ZERO;
        while waited < grace && live.iter().any(|a| a.group_alive()) {
            std::thread::sleep(step);
            waited += step;
        }
        for app in &live {
            let _ = app.signal(Signal::KILL);
        }
    }
}
