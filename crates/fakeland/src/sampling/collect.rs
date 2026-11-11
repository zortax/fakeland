//! The read of every launched tree at one sample: membership from one `/proc` snapshot, then the
//! CPU, context switch, memory and thread counters of each member, summed per tree.

use std::collections::{BTreeMap, BTreeSet};

use fakeland_types::Timestamp;

use super::cpu::{CpuAccount, Member};
use super::procfs::{StatLine, read_all_stats, read_status, read_threads, read_uptime};
use super::smaps::{SmapsRollup, read_smaps_rollup};
use super::switches::{MemberSwitches, SwitchAccount};
use super::tree::members;

/// The reading of one tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeReading {
    /// The root pid.
    pub root: u32,
    /// CPU use in percent of one core, `None` on the first sample of the tree.
    pub cpu_percent: Option<f64>,
    /// The time on CPU of every member since the first sample of the tree, in nanoseconds.
    pub cpu_ns_total: u64,
    /// The context switches of every member thread since the first sample of the tree.
    pub ctx_switches_total: u64,
    /// Resident memory in bytes.
    pub rss_bytes: u64,
    /// The memory rollup over every member, `None` when off or unreadable.
    pub memory: Option<SmapsRollup>,
    /// The largest peak resident set of a member in bytes, `None` when off or unread.
    pub hwm_bytes: Option<u64>,
    /// Threads.
    pub threads: u32,
    /// Member processes.
    pub processes: u32,
}

/// The launched roots and the counter state of their trees.
#[derive(Debug, Default)]
pub struct Trees {
    /// The launched roots.
    pub roots: BTreeSet<u32>,
    /// Roots that exited. Their trees drop once no member lives.
    pub exited: BTreeSet<u32>,
    cpu: CpuAccount,
    switches: SwitchAccount,
}

/// The result of one read.
#[derive(Debug, Default)]
pub struct TreeSample {
    /// The tree readings.
    pub readings: Vec<TreeReading>,
    /// The member pids of each live tree.
    pub members: Vec<(u32, Vec<u32>)>,
    /// Roots whose trees ended.
    pub ended: Vec<u32>,
}

impl Trees {
    /// Reads every tree at time `t`. Reads the memory rollup and the peak resident set only when
    /// `memory_detail` is true.
    pub fn sample(&mut self, t: Timestamp, memory_detail: bool) -> TreeSample {
        let stats = read_all_stats();
        let uptime = read_uptime().unwrap_or(0);
        let by_root = members(&stats, &self.roots);
        let stat_of: BTreeMap<u32, &StatLine> = stats.iter().map(|s| (s.pid, s)).collect();
        let mut out = TreeSample::default();
        for (root, pids) in by_root {
            if pids.is_empty() {
                if self.exited.contains(&root) {
                    out.ended.push(root);
                }
                continue;
            }
            let mut cpu = Vec::with_capacity(pids.len());
            let mut switches = Vec::with_capacity(pids.len());
            let (mut rss, mut threads) = (0u64, 0u32);
            let (mut memory, mut hwm) = (None::<SmapsRollup>, None::<u64>);
            for pid in &pids {
                let Some(stat) = stat_of.get(pid) else {
                    continue;
                };
                let counters = read_threads(*pid, true);
                cpu.push(Member {
                    pid: *pid,
                    starttime: stat.starttime,
                    ticks: stat.ticks(),
                    threads: counters
                        .iter()
                        .filter_map(|c| Some((c.tid, c.runtime_ns?)))
                        .collect(),
                });
                switches.push(MemberSwitches {
                    pid: *pid,
                    starttime: stat.starttime,
                    threads: counters
                        .iter()
                        .filter_map(|c| Some((c.tid, c.switches?)))
                        .collect(),
                });
                if let Some(status) = read_status(*pid) {
                    rss += status.rss_bytes;
                    threads += status.threads;
                    if memory_detail && let Some(h) = status.hwm_bytes {
                        hwm = Some(hwm.unwrap_or(0).max(h));
                    }
                }
                if memory_detail && let Some(r) = read_smaps_rollup(*pid) {
                    memory = Some(memory.unwrap_or_default().add(r));
                }
            }
            let cpu_percent = self.cpu.update(root, t, uptime, &cpu);
            out.readings.push(TreeReading {
                root,
                cpu_percent,
                cpu_ns_total: self.cpu.total_ns(root).unwrap_or(0),
                ctx_switches_total: self.switches.update(root, uptime, &switches),
                rss_bytes: rss,
                memory,
                hwm_bytes: hwm,
                threads,
                processes: cpu.len() as u32,
            });
            out.members.push((root, pids));
        }
        for root in &out.ended {
            self.roots.remove(root);
            self.exited.remove(root);
            self.cpu.forget(*root);
            self.switches.forget(*root);
        }
        out
    }
}
