//! Process tree lookups in `/proc`: the parent, process group and session of a pid, the launched
//! ancestor of a client process, and the live members of a process group.

use std::collections::BTreeSet;
use std::fs;

/// The fields of `/proc/<pid>/stat` that matter for ancestry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcStat {
    /// The parent pid.
    pub ppid: u32,
    /// The process group id.
    pub pgrp: u32,
    /// The session id.
    pub session: u32,
}

/// Parses the content of `/proc/<pid>/stat`. The command name sits in parentheses and may hold
/// spaces and parentheses, so parsing starts after the last `)`.
pub fn parse_stat(content: &str) -> Option<ProcStat> {
    let rest = &content[content.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    let _state = fields.next()?;
    let ppid = fields.next()?.parse().ok()?;
    let pgrp = fields.next()?.parse().ok()?;
    let session = fields.next()?.parse().ok()?;
    Some(ProcStat {
        ppid,
        pgrp,
        session,
    })
}

/// Reads the stat of a live process.
pub fn read_stat(pid: u32) -> Option<ProcStat> {
    parse_stat(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// Returns the session of this process.
pub fn own_session() -> Option<u32> {
    read_stat(std::process::id()).map(|s| s.session)
}

/// Returns true when a process of group `pgrp` lives in session `session`.
pub fn group_has_member(
    stats: impl IntoIterator<Item = ProcStat>,
    pgrp: u32,
    session: u32,
) -> bool {
    stats
        .into_iter()
        .any(|s| s.pgrp == pgrp && s.session == session)
}

/// Returns the stat of every live process.
pub fn all_stats() -> Vec<ProcStat> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(read_stat)
        .collect()
}

/// Finds the root among `roots` that `pid` descends from, through the parent chain or the
/// process group.
pub fn find_root(
    pid: u32,
    roots: &BTreeSet<u32>,
    stat: impl Fn(u32) -> Option<ProcStat>,
) -> Option<u32> {
    let mut current = pid;
    let mut first_group = None;
    for _ in 0..64 {
        if roots.contains(&current) {
            return Some(current);
        }
        let Some(s) = stat(current) else { break };
        first_group.get_or_insert(s.pgrp);
        if s.ppid <= 1 {
            break;
        }
        current = s.ppid;
    }
    first_group.filter(|g| roots.contains(g))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn parses_names_with_parentheses() {
        let s = "1234 (Web Content (x)) S 1200 1100 1000 0 -1 4194560";
        assert_eq!(
            parse_stat(s),
            Some(ProcStat {
                ppid: 1200,
                pgrp: 1100,
                session: 1000,
            })
        );
        assert_eq!(parse_stat("garbage"), None);
    }

    fn stat(ppid: u32, pgrp: u32) -> ProcStat {
        ProcStat {
            ppid,
            pgrp,
            session: 5,
        }
    }

    #[test]
    fn walks_parents_and_groups() {
        let table: BTreeMap<u32, ProcStat> = [
            (30, stat(20, 10)),
            (20, stat(10, 10)),
            (10, stat(1, 10)),
            (50, stat(1, 40)),
        ]
        .into_iter()
        .collect();
        let stat = |p: u32| table.get(&p).copied();
        let roots: BTreeSet<u32> = [10, 40].into_iter().collect();
        assert_eq!(find_root(30, &roots, stat), Some(10));
        assert_eq!(find_root(50, &roots, stat), Some(40));
        assert_eq!(find_root(99, &roots, stat), None);
    }

    #[test]
    fn group_members_count_only_in_the_session() {
        let table = [stat(1, 10), stat(10, 10), stat(1, 40)];
        assert!(group_has_member(table, 10, 5));
        assert!(!group_has_member(table, 20, 5));
        assert!(!group_has_member(table, 10, 6));
    }

    #[test]
    fn reads_own_stat() {
        let me = std::process::id();
        let stat = read_stat(me).unwrap();
        assert!(stat.ppid > 0);
        assert_eq!(own_session(), Some(stat.session));
        assert!(group_has_member(all_stats(), stat.pgrp, stat.session));
    }
}
