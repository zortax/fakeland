//! Process tree membership from one `/proc` snapshot. A process belongs to the tree of a launched
//! root when its parent chain reaches the root or when it sits in the root's process group. The
//! group catches children that outlive their parent, since the launcher starts each app in a
//! group of its own.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::procfs::StatLine;

/// The deepest parent chain the walk follows.
const MAX_DEPTH: usize = 64;

/// Returns the members of each root's tree, the root first when it lives.
pub fn members(stats: &[StatLine], roots: &BTreeSet<u32>) -> BTreeMap<u32, Vec<u32>> {
    let by_pid: HashMap<u32, &StatLine> = stats.iter().map(|s| (s.pid, s)).collect();
    let mut out: BTreeMap<u32, Vec<u32>> = roots.iter().map(|&r| (r, Vec::new())).collect();
    for s in stats {
        if let Some(root) = root_of(s, &by_pid, roots) {
            out.entry(root).or_default().push(s.pid);
        }
    }
    for (root, list) in &mut out {
        list.sort_unstable_by_key(|&p| (p != *root, p));
    }
    out
}

fn root_of(s: &StatLine, by_pid: &HashMap<u32, &StatLine>, roots: &BTreeSet<u32>) -> Option<u32> {
    let mut current = s;
    for _ in 0..MAX_DEPTH {
        if roots.contains(&current.pid) {
            return Some(current.pid);
        }
        if current.ppid <= 1 {
            break;
        }
        match by_pid.get(&current.ppid) {
            Some(parent) => current = parent,
            None => break,
        }
    }
    roots.contains(&s.pgrp).then_some(s.pgrp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(pid: u32, ppid: u32, pgrp: u32) -> StatLine {
        StatLine {
            pid,
            ppid,
            pgrp,
            utime: 0,
            stime: 0,
            cutime: 0,
            cstime: 0,
            starttime: 0,
        }
    }

    #[test]
    fn collects_descendants_and_group_members() {
        let stats = vec![
            stat(1, 0, 1),
            stat(10, 1, 10),
            stat(11, 10, 10),
            stat(12, 11, 12),
            stat(13, 1, 10),
            stat(20, 1, 20),
            stat(21, 20, 20),
            stat(30, 1, 30),
        ];
        let roots: BTreeSet<u32> = [10, 20, 99].into_iter().collect();
        let m = members(&stats, &roots);
        assert_eq!(m[&10], vec![10, 11, 12, 13]);
        assert_eq!(m[&20], vec![20, 21]);
        assert!(m[&99].is_empty());
        assert!(!m.values().flatten().any(|&p| p == 30 || p == 1));
    }

    #[test]
    fn a_nested_root_owns_its_own_tree() {
        let stats = vec![stat(10, 1, 10), stat(11, 10, 10), stat(12, 11, 10)];
        let roots: BTreeSet<u32> = [10, 11].into_iter().collect();
        let m = members(&stats, &roots);
        assert_eq!(m[&10], vec![10]);
        assert_eq!(m[&11], vec![11, 12]);
    }
}
