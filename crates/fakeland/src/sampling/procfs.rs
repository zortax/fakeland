//! Reads of `/proc`: the stat line of every process, the memory, thread and context switch lines
//! of `/proc/<pid>/status`, and the counters of each thread. Parsers take text, so tests feed them
//! fixed content.
//!
//! The context switch lines of `/proc/<pid>/status` count the main thread only. The switches of a
//! process are the sum over the status files of its threads in `/proc/<pid>/task`.

use std::fs;

/// Clock ticks per second of the CPU times in `/proc/<pid>/stat`. Linux fixes `USER_HZ` at 100
/// on every architecture that fakeland runs on.
pub const USER_HZ: f64 = 100.0;

/// The fields of `/proc/<pid>/stat` that sampling needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatLine {
    /// The pid.
    pub pid: u32,
    /// The parent pid.
    pub ppid: u32,
    /// The process group id.
    pub pgrp: u32,
    /// User CPU time in ticks.
    pub utime: u64,
    /// System CPU time in ticks.
    pub stime: u64,
    /// User CPU time of waited-for children in ticks.
    pub cutime: u64,
    /// System CPU time of waited-for children in ticks.
    pub cstime: u64,
    /// The start time after boot in ticks. A pid with a new start time is a new process.
    pub starttime: u64,
}

impl StatLine {
    /// Returns the CPU ticks of the process and of its waited-for children.
    pub fn ticks(&self) -> u64 {
        self.utime + self.stime + self.cutime + self.cstime
    }
}

/// Parses a stat line. The command name sits in parentheses and may hold spaces and
/// parentheses, so the fields after the name start after the last `)`.
pub fn parse_stat(content: &str) -> Option<StatLine> {
    let open = content.find('(')?;
    let pid = content[..open].trim().parse().ok()?;
    let rest = &content[content.rfind(')')? + 1..];
    // Fields after the name, counted from 0: state(0) ppid(1) pgrp(2) ... utime(11) stime(12)
    // cutime(13) cstime(14) ... starttime(19).
    let f: Vec<&str> = rest.split_whitespace().collect();
    let num = |i: usize| f.get(i)?.parse::<u64>().ok();
    let signed = |i: usize| f.get(i)?.parse::<i64>().ok().map(|v| v.max(0) as u64);
    Some(StatLine {
        pid,
        ppid: f.get(1)?.parse().ok()?,
        pgrp: f.get(2)?.parse().ok()?,
        utime: num(11)?,
        stime: num(12)?,
        cutime: signed(13)?,
        cstime: signed(14)?,
        starttime: num(19)?,
    })
}

/// The fields of `/proc/<pid>/status` that sampling needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusLines {
    /// Resident memory in bytes.
    pub rss_bytes: u64,
    /// The thread count.
    pub threads: u32,
    /// The peak resident memory in bytes, `None` without a `VmHWM` line.
    pub hwm_bytes: Option<u64>,
    /// The voluntary and involuntary context switches of the task, `None` without both lines.
    pub ctx_switches: Option<u64>,
}

/// Parses the `VmRSS`, `VmHWM`, `Threads` and context switch lines of a status file. A kernel
/// thread or a zombie has no `VmRSS` line, which reads as zero.
pub fn parse_status(content: &str) -> StatusLines {
    let mut out = StatusLines::default();
    let (mut voluntary, mut involuntary) = (None, None);
    for line in content.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            "VmRSS" => out.rss_bytes = parse_kb(value).unwrap_or(0),
            "VmHWM" => out.hwm_bytes = parse_kb(value),
            "Threads" => out.threads = value.parse().unwrap_or(0),
            "voluntary_ctxt_switches" => voluntary = value.parse::<u64>().ok(),
            "nonvoluntary_ctxt_switches" => involuntary = value.parse::<u64>().ok(),
            _ => {}
        }
    }
    out.ctx_switches = voluntary.zip(involuntary).map(|(v, i)| v + i);
    out
}

/// Parses a value of the form `51200 kB` into bytes.
pub fn parse_kb(value: &str) -> Option<u64> {
    let kb: u64 = value.split_whitespace().next()?.parse().ok()?;
    Some(kb * 1024)
}

/// Parses `/proc/uptime` into clock ticks since boot.
pub fn parse_uptime(content: &str) -> Option<u64> {
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    (secs.is_finite() && secs >= 0.0).then_some((secs * USER_HZ) as u64)
}

/// Reads the time since boot in clock ticks.
pub fn read_uptime() -> Option<u64> {
    parse_uptime(&fs::read_to_string("/proc/uptime").ok()?)
}

/// Parses a `schedstat` line into the time on CPU in nanoseconds.
pub fn parse_schedstat(content: &str) -> Option<u64> {
    content.split_whitespace().next()?.parse().ok()
}

/// The counters of one live thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadCounters {
    /// The thread id.
    pub tid: u32,
    /// The time on CPU in nanoseconds, `None` when the kernel has no scheduler statistics.
    pub runtime_ns: Option<u64>,
    /// The voluntary and involuntary context switches, `None` when unread.
    pub switches: Option<u64>,
}

/// Reads the counters of every live thread of a process. Reads the context switches only when
/// `switches` is true.
pub fn read_threads(pid: u32, switches: bool) -> Vec<ThreadCounters> {
    let Ok(dir) = fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let tid: u32 = e.file_name().to_str()?.parse().ok()?;
            let path = e.path();
            let runtime_ns = fs::read_to_string(path.join("schedstat"))
                .ok()
                .and_then(|t| parse_schedstat(&t));
            let switches = switches
                .then(|| fs::read_to_string(path.join("status")).ok())
                .flatten()
                .and_then(|t| parse_status(&t).ctx_switches);
            Some(ThreadCounters {
                tid,
                runtime_ns,
                switches,
            })
        })
        .collect()
}

/// Reads the stat line of every live process.
pub fn read_all_stats() -> Vec<StatLine> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(read_stat)
        .collect()
}

/// Reads the stat line of one process.
pub fn read_stat(pid: u32) -> Option<StatLine> {
    parse_stat(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// Reads the status lines of one process.
pub fn read_status(pid: u32) -> Option<StatusLines> {
    Some(parse_status(
        &fs::read_to_string(format!("/proc/{pid}/status")).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "4242 (Web (Content) x) S 4200 4100 4100 0 -1 4194560 100 0 0 0 \
                        250 75 10 5 20 0 12 0 98765 1000000 2000 18446744073709551615";

    #[test]
    fn parses_stat_after_the_last_parenthesis() {
        let s = parse_stat(STAT).unwrap();
        assert_eq!(s.pid, 4242);
        assert_eq!((s.ppid, s.pgrp), (4200, 4100));
        assert_eq!((s.utime, s.stime, s.cutime, s.cstime), (250, 75, 10, 5));
        assert_eq!(s.starttime, 98765);
        assert_eq!(s.ticks(), 340);
        assert_eq!(parse_stat("garbage"), None);
        assert_eq!(parse_stat("12 (x) S 1"), None);
    }

    #[test]
    fn parses_status_memory_and_threads() {
        let text = "Name:\tvkcube\nVmPeak:\t  999 kB\nVmHWM:\t   60000 kB\nVmRSS:\t   51200 kB\n\
                    Threads:\t7\nvoluntary_ctxt_switches:\t1200\n\
                    nonvoluntary_ctxt_switches:\t34\n";
        assert_eq!(
            parse_status(text),
            StatusLines {
                rss_bytes: 51200 * 1024,
                threads: 7,
                hwm_bytes: Some(60000 * 1024),
                ctx_switches: Some(1234),
            }
        );
        assert_eq!(parse_status("Name:\tkworker\n"), StatusLines::default());
        let half = parse_status("voluntary_ctxt_switches:\t5\n");
        assert_eq!(half.ctx_switches, None);
    }

    #[test]
    fn parses_uptime_in_ticks() {
        assert_eq!(parse_uptime("1234.56 999.00\n"), Some(123456));
        assert_eq!(parse_uptime("x"), None);
        assert!(read_uptime().unwrap() > 0);
    }

    #[test]
    fn parses_schedstat_runtime() {
        assert_eq!(parse_schedstat("1418064 9144 2\n"), Some(1418064));
        assert_eq!(parse_schedstat(""), None);
    }

    #[test]
    fn reads_own_process() {
        let me = std::process::id();
        let stat = read_stat(me).unwrap();
        assert_eq!(stat.pid, me);
        let status = read_status(me).unwrap();
        assert!(status.rss_bytes > 0);
        assert!(status.threads >= 1);
        assert!(read_all_stats().iter().any(|s| s.pid == me));
        let threads = read_threads(me, true);
        let main = threads.iter().find(|t| t.tid == me).unwrap();
        assert!(main.runtime_ns.unwrap() > 0);
        assert!(main.switches.is_some());
        assert!(read_threads(me, false).iter().all(|t| t.switches.is_none()));
    }
}
