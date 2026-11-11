//! Cumulative context switches of a process tree. The kernel counts switches per thread, and the
//! count of a thread disappears when the thread ends. The tree total grows by the switches of each
//! live thread since the previous sample, so it never falls; a thread that ends inside a span
//! loses only its switches of that span. A thread that starts inside the span counts in full.
//!
//! A process keys by pid and start time. A process without a baseline that started before the
//! previous sample joins the tree from outside and counts from the next sample on.

use std::collections::{HashMap, HashSet};

/// The switches of the threads of one member at one sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberSwitches {
    /// The pid.
    pub pid: u32,
    /// The start time after boot in ticks.
    pub starttime: u64,
    /// The switches of each live thread, by thread id.
    pub threads: Vec<(u32, u64)>,
}

#[derive(Debug, Default)]
struct Baseline {
    uptime: u64,
    processes: HashSet<(u32, u64)>,
    threads: HashMap<(u32, u64, u32), u64>,
    total: u64,
}

/// The previous switch counts of every tree.
#[derive(Debug, Default)]
pub struct SwitchAccount {
    trees: HashMap<u32, Baseline>,
}

impl SwitchAccount {
    /// Takes a sample of the tree of `root` at `uptime` ticks after boot and returns the
    /// cumulative switches. The first sample starts at the sum of the live thread counts.
    pub fn update(&mut self, root: u32, uptime: u64, members: &[MemberSwitches]) -> u64 {
        let mut next = Baseline {
            uptime,
            ..Baseline::default()
        };
        for m in members {
            next.processes.insert((m.pid, m.starttime));
            for &(tid, n) in &m.threads {
                next.threads.insert((m.pid, m.starttime, tid), n);
            }
        }
        next.total = match self.trees.get(&root) {
            None => members.iter().flat_map(|m| &m.threads).map(|t| t.1).sum(),
            Some(prev) => prev.total + delta(prev, members),
        };
        let total = next.total;
        self.trees.insert(root, next);
        total
    }

    /// Drops the baseline of a tree.
    pub fn forget(&mut self, root: u32) {
        self.trees.remove(&root);
    }
}

fn delta(prev: &Baseline, members: &[MemberSwitches]) -> u64 {
    let mut sum = 0;
    for m in members {
        let known = prev.processes.contains(&(m.pid, m.starttime));
        if !known && m.starttime < prev.uptime {
            continue;
        }
        for &(tid, n) in &m.threads {
            sum += match prev.threads.get(&(m.pid, m.starttime, tid)) {
                Some(&before) => n.saturating_sub(before),
                None => n,
            };
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pid: u32, starttime: u64, threads: &[(u32, u64)]) -> MemberSwitches {
        MemberSwitches {
            pid,
            starttime,
            threads: threads.to_vec(),
        }
    }

    #[test]
    fn counts_grow_over_exited_threads_and_new_members() {
        let mut acc = SwitchAccount::default();
        assert_eq!(
            acc.update(10, 1000, &[m(10, 5, &[(10, 100), (11, 50)])]),
            150
        );
        // Thread 11 ends; thread 10 switches 20 times; child 20 starts inside the span.
        let n = acc.update(10, 1100, &[m(10, 5, &[(10, 120)]), m(20, 1050, &[(20, 7)])]);
        assert_eq!(n, 177);
        // A new thread of a known process counts in full.
        let n = acc.update(
            10,
            1200,
            &[m(10, 5, &[(10, 121), (12, 3)]), m(20, 1050, &[(20, 7)])],
        );
        assert_eq!(n, 181);
    }

    #[test]
    fn old_joiners_and_reused_pids_wait_a_round() {
        let mut acc = SwitchAccount::default();
        acc.update(10, 1000, &[m(10, 5, &[(10, 100)])]);
        let n = acc.update(10, 1100, &[m(10, 5, &[(10, 100)]), m(30, 7, &[(30, 9000)])]);
        assert_eq!(n, 100);
        let n = acc.update(10, 1200, &[m(10, 5, &[(10, 100)]), m(30, 7, &[(30, 9010)])]);
        assert_eq!(n, 110);
        // Pid 10 is reused by a process that started after the previous sample.
        let n = acc.update(10, 1300, &[m(10, 1250, &[(10, 4)])]);
        assert_eq!(n, 114);
        acc.forget(10);
        assert_eq!(acc.update(10, 1400, &[m(10, 1250, &[(10, 5)])]), 5);
    }
}
