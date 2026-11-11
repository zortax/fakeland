//! CPU accounting of a process tree between two samples, from two counters.
//!
//! The tick counter adds the ticks of every member and of its waited-for children. A child that
//! exits and gets reaped inside the tree moves its whole lifetime into the parent, so the tree
//! subtracts the baseline of every member that disappears, and the child's time counts once.
//! Ticks are 10 ms, so one sample can be off by one tick per member.
//!
//! The runtime counter adds the nanosecond time on CPU of every live thread. It is exact for
//! threads that live through the span and misses the last slice of threads that end inside it.
//!
//! A sample uses the runtime counter, unless the tick counter exceeds it by more than its own
//! rounding, which means that exited threads or reaped children carried time away. A process
//! keys by pid and start time, so a reused pid starts a new process.

use std::collections::HashMap;

use fakeland_types::Timestamp;

use super::procfs::USER_HZ;

/// Nanoseconds per clock tick.
const TICK_NS: f64 = 1e9 / USER_HZ;

/// One member of a tree at one sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// The pid.
    pub pid: u32,
    /// The start time after boot in ticks.
    pub starttime: u64,
    /// The CPU ticks of the process and its waited-for children.
    pub ticks: u64,
    /// The time on CPU of each live thread in nanoseconds, by thread id.
    pub threads: Vec<(u32, u64)>,
}

#[derive(Debug, Default)]
struct Baseline {
    t: Timestamp,
    uptime: u64,
    ticks: HashMap<(u32, u64), u64>,
    runtime: HashMap<(u32, u64, u32), u64>,
}

impl Baseline {
    fn of(t: Timestamp, uptime: u64, members: &[Member]) -> Self {
        let mut runtime = HashMap::new();
        for m in members {
            for &(tid, ns) in &m.threads {
                runtime.insert((m.pid, m.starttime, tid), ns);
            }
        }
        Self {
            t,
            uptime,
            ticks: members
                .iter()
                .map(|m| ((m.pid, m.starttime), m.ticks))
                .collect(),
            runtime,
        }
    }
}

/// The previous sample of every tree.
#[derive(Debug, Default)]
pub struct CpuAccount {
    trees: HashMap<u32, Baseline>,
}

impl CpuAccount {
    /// Takes a sample of the tree of `root` at time `t` and `uptime` ticks after boot. Returns
    /// the CPU use since the previous sample in percent of one core, or `None` for the first
    /// sample of the tree.
    pub fn update(
        &mut self,
        root: u32,
        t: Timestamp,
        uptime: u64,
        members: &[Member],
    ) -> Option<f64> {
        let prev = self.trees.insert(root, Baseline::of(t, uptime, members))?;
        let span = t.since(prev.t).as_secs_f64();
        if span <= 0.0 {
            return Some(0.0);
        }
        let ticks_ns = tick_delta(&prev, &self.trees[&root], members) as f64 * TICK_NS;
        let has_runtime = members.iter().any(|m| !m.threads.is_empty());
        let ns = if has_runtime {
            let runtime_ns = runtime_delta(&prev, members) as f64;
            let rounding = (members.len() as f64 + 1.0) * TICK_NS;
            if ticks_ns > runtime_ns + rounding {
                ticks_ns
            } else {
                runtime_ns
            }
        } else {
            ticks_ns
        };
        Some(ns / 1e9 / span * 100.0)
    }

    /// Drops the baseline of a tree.
    pub fn forget(&mut self, root: u32) {
        self.trees.remove(&root);
    }
}

/// Returns true when a member without a baseline started inside the span. An older process that
/// joins the tree has no baseline and waits a round.
fn born_inside(prev: &Baseline, m: &Member) -> bool {
    m.starttime >= prev.uptime
}

fn tick_delta(prev: &Baseline, next: &Baseline, members: &[Member]) -> u64 {
    let grown: i64 = members
        .iter()
        .map(|m| match prev.ticks.get(&(m.pid, m.starttime)) {
            Some(&before) => m.ticks as i64 - before as i64,
            None if born_inside(prev, m) => m.ticks as i64,
            None => 0,
        })
        .sum();
    let gone: i64 = prev
        .ticks
        .iter()
        .filter(|(key, _)| !next.ticks.contains_key(key))
        .map(|(_, &t)| t as i64)
        .sum();
    (grown - gone).max(0) as u64
}

fn runtime_delta(prev: &Baseline, members: &[Member]) -> u64 {
    let mut sum = 0u64;
    for m in members {
        let known = prev.ticks.contains_key(&(m.pid, m.starttime));
        if !known && !born_inside(prev, m) {
            continue;
        }
        for &(tid, ns) in &m.threads {
            sum += match prev.runtime.get(&(m.pid, m.starttime, tid)) {
                Some(&before) => ns.saturating_sub(before),
                None => ns,
            };
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pid: u32, starttime: u64, ticks: u64) -> Member {
        Member {
            pid,
            starttime,
            ticks,
            threads: Vec::new(),
        }
    }

    fn mt(pid: u32, starttime: u64, ticks: u64, threads: &[(u32, u64)]) -> Member {
        Member {
            threads: threads.to_vec(),
            ..m(pid, starttime, ticks)
        }
    }

    const SEC: u64 = 1_000_000_000;
    const MS: u64 = 1_000_000;

    #[test]
    fn first_sample_is_a_baseline() {
        let mut acc = CpuAccount::default();
        assert_eq!(acc.update(10, Timestamp(SEC), 1000, &[m(10, 5, 100)]), None);
    }

    #[test]
    fn one_busy_core_reads_one_hundred_percent() {
        let mut acc = CpuAccount::default();
        acc.update(10, Timestamp(SEC), 1000, &[m(10, 5, 100)]);
        let cpu = acc
            .update(10, Timestamp(2 * SEC), 1100, &[m(10, 5, 200)])
            .unwrap();
        assert!((cpu - 100.0).abs() < 1e-9, "{cpu}");
    }

    #[test]
    fn reaped_children_count_once_and_new_children_count_fully() {
        let mut acc = CpuAccount::default();
        acc.update(10, Timestamp(SEC), 1000, &[m(10, 5, 100), m(11, 900, 40)]);
        // Child 11 ran 10 more ticks, exited and got reaped: its 50 ticks moved to the parent.
        // Child 12 started inside the span and ran 30 ticks.
        let cpu = acc
            .update(
                10,
                Timestamp(SEC + SEC / 2),
                1050,
                &[m(10, 5, 100 + 5 + 50), m(12, 1010, 30)],
            )
            .unwrap();
        // Parent 5 + reaped child 10 + new child 30 = 45 ticks in 0.5 s.
        assert!((cpu - 90.0).abs() < 1e-9, "{cpu}");
    }

    #[test]
    fn a_child_reaped_outside_the_tree_never_goes_negative() {
        let mut acc = CpuAccount::default();
        acc.update(10, Timestamp(SEC), 1000, &[m(10, 5, 100), m(11, 900, 400)]);
        let cpu = acc
            .update(10, Timestamp(2 * SEC), 1100, &[m(10, 5, 110)])
            .unwrap();
        assert_eq!(cpu, 0.0);
    }

    #[test]
    fn old_processes_without_baseline_and_reused_pids_wait_a_round() {
        let mut acc = CpuAccount::default();
        acc.update(10, Timestamp(SEC), 1000, &[m(10, 5, 100)]);
        let cpu = acc
            .update(
                10,
                Timestamp(2 * SEC),
                1100,
                &[m(10, 5, 100), mt(13, 7, 5000, &[(13, 50 * SEC)])],
            )
            .unwrap();
        assert_eq!(cpu, 0.0);
        acc.forget(10);
        assert_eq!(
            acc.update(10, Timestamp(3 * SEC), 1200, &[m(10, 5, 100)]),
            None
        );
    }

    #[test]
    fn thread_runtime_gives_sub_tick_precision() {
        let mut acc = CpuAccount::default();
        let t0 = Timestamp(SEC);
        acc.update(10, t0, 1000, &[mt(10, 5, 100, &[(10, 1000 * MS), (11, 0)])]);
        // 4.2 ms in 100 ms: the ticks moved by zero, the threads by 3 ms and 1.2 ms.
        let cpu = acc
            .update(
                10,
                t0 + fakeland_types::Nanos::from_millis(100),
                1010,
                &[mt(10, 5, 100, &[(10, 1003 * MS), (11, 1200 * 1000)])],
            )
            .unwrap();
        assert!((cpu - 4.2).abs() < 1e-9, "{cpu}");
    }

    #[test]
    fn ticks_win_when_exited_threads_carry_time_away() {
        let mut acc = CpuAccount::default();
        let t0 = Timestamp(SEC);
        acc.update(10, t0, 1000, &[mt(10, 5, 100, &[(10, 0), (12, 0)])]);
        // Thread 12 burned 500 ms and ended; the process ticks hold it, the live threads do not.
        let cpu = acc
            .update(
                10,
                t0 + fakeland_types::Nanos::from_secs(1),
                1100,
                &[mt(10, 5, 151, &[(10, 10 * MS)])],
            )
            .unwrap();
        assert!((cpu - 51.0).abs() < 1e-9, "{cpu}");
    }
}
