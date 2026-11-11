//! Host load from `/proc`: the CPU time of the whole system, of this process and of the apps
//! under test, sampled before and after a run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// CPU tick counters at one point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostSample {
    /// The ticks of every CPU in every state.
    pub total: u64,
    /// The idle and iowait ticks of every CPU.
    pub idle: u64,
    /// The user and system ticks of this process.
    pub own: u64,
    /// The user and system ticks of the apps under test, by pid.
    pub apps: BTreeMap<u32, u64>,
    /// The count of CPUs.
    pub cpus: usize,
    /// The one-minute load average.
    pub loadavg: Option<f64>,
}

impl HostSample {
    /// Reads the counters. Returns `None` without `/proc/stat`.
    pub fn read(apps: &[u32]) -> Option<Self> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        let (total, idle, cpus) = parse_stat(&stat)?;
        let own = std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|s| parse_pid_ticks(&s))
            .unwrap_or(0);
        let apps = apps
            .iter()
            .filter_map(|pid| {
                let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
                Some((*pid, parse_pid_ticks(&s)?))
            })
            .collect();
        let loadavg = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse().ok());
        Some(Self {
            total,
            idle,
            own,
            apps,
            cpus,
            loadavg,
        })
    }
}

/// The host load between two samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostLoad {
    /// The busy time of every CPU as a fraction of all CPU time.
    pub busy: f64,
    /// The busy time without this process and the apps under test.
    pub other_busy: f64,
    /// The count of CPUs.
    pub cpus: usize,
    /// The one-minute load average at the end.
    pub loadavg: Option<f64>,
}

impl HostLoad {
    /// Computes the load between two samples. Returns `None` when no time passed.
    pub fn between(a: &HostSample, b: &HostSample) -> Option<Self> {
        let total = b.total.checked_sub(a.total).filter(|t| *t > 0)? as f64;
        let idle = b.idle.saturating_sub(a.idle) as f64;
        let busy = (total - idle).max(0.0);
        let own = b.own.saturating_sub(a.own) as f64;
        let apps: f64 = b
            .apps
            .iter()
            .map(|(pid, t)| t.saturating_sub(a.apps.get(pid).copied().unwrap_or(0)) as f64)
            .sum();
        Some(Self {
            busy: (busy / total).clamp(0.0, 1.0),
            other_busy: ((busy - own - apps) / total).clamp(0.0, 1.0),
            cpus: b.cpus,
            loadavg: b.loadavg,
        })
    }
}

/// Parses the aggregate `cpu` line of `/proc/stat` into total ticks, idle ticks and the CPU
/// count.
pub(crate) fn parse_stat(text: &str) -> Option<(u64, u64, usize)> {
    let mut lines = text.lines();
    let cpu = lines.next()?.strip_prefix("cpu ")?;
    let fields: Vec<u64> = cpu
        .split_whitespace()
        .filter_map(|f| f.parse().ok())
        .collect();
    if fields.len() < 4 {
        return None;
    }
    // Guest time is part of user time already.
    let total: u64 = fields.iter().take(8).sum();
    let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
    let cpus = text
        .lines()
        .filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
        .count();
    Some((total, idle, cpus.max(1)))
}

/// Parses the user and system ticks of `/proc/<pid>/stat`.
pub(crate) fn parse_pid_ticks(text: &str) -> Option<u64> {
    let rest = &text[text.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After the command come state (field 3), so utime (14) and stime (15) sit at 11 and 12.
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  100 5 50 800 20 3 2 0 0 0\ncpu0 50 2 25 400 10 1 1 0 0 0\n\
                        cpu1 50 3 25 400 10 2 1 0 0 0\nintr 1 2 3\n";

    #[test]
    fn parses_proc_stat() {
        assert_eq!(parse_stat(STAT), Some((980, 820, 2)));
        assert_eq!(parse_stat("garbage"), None);
    }

    #[test]
    fn parses_pid_stat_with_spaces_in_the_name() {
        let s = "1234 (my (odd) app) S 1 1234 1234 0 -1 4194560 100 0 0 0 70 30 0 0 20 0 4 0";
        assert_eq!(parse_pid_ticks(s), Some(100));
    }

    #[test]
    fn load_excludes_own_and_app_time() {
        let a = HostSample {
            total: 1000,
            idle: 900,
            own: 10,
            apps: [(7, 5)].into(),
            cpus: 4,
            loadavg: None,
        };
        let b = HostSample {
            total: 2000,
            idle: 1500,
            own: 110,
            apps: [(7, 105)].into(),
            cpus: 4,
            loadavg: Some(1.5),
        };
        let l = HostLoad::between(&a, &b).unwrap();
        assert!((l.busy - 0.4).abs() < 1e-12);
        assert!((l.other_busy - 0.2).abs() < 1e-12);
        assert_eq!(l.loadavg, Some(1.5));
        assert!(HostLoad::between(&b, &b).is_none());
    }

    #[test]
    fn reads_the_live_host() {
        let s = HostSample::read(&[std::process::id()]).expect("procfs");
        assert!(s.total > 0 && s.cpus >= 1);
        assert!(s.apps.contains_key(&std::process::id()));
    }
}
