//! Memory detail of a process from `/proc/<pid>/smaps_rollup`: the proportional set size, the
//! private memory and the swapped memory. The kernel walks every mapping for this file, so it
//! costs more than the status file. A process of another user or a kernel thread has no readable
//! rollup.

use std::fs;

use super::procfs::parse_kb;

/// The rollup lines that sampling needs, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmapsRollup {
    /// The proportional set size: each shared page counts divided by its sharers.
    pub pss_bytes: u64,
    /// Private clean and private dirty memory.
    pub private_bytes: u64,
    /// Swapped memory.
    pub swap_bytes: u64,
}

impl SmapsRollup {
    /// Returns the sum of two rollups.
    pub fn add(self, other: Self) -> Self {
        Self {
            pss_bytes: self.pss_bytes + other.pss_bytes,
            private_bytes: self.private_bytes + other.private_bytes,
            swap_bytes: self.swap_bytes + other.swap_bytes,
        }
    }
}

/// Parses a rollup. Returns `None` when the text has no `Pss` line.
pub fn parse_smaps_rollup(content: &str) -> Option<SmapsRollup> {
    let mut out = SmapsRollup::default();
    let mut pss = false;
    for line in content.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Some(bytes) = parse_kb(value) else {
            continue;
        };
        match key {
            "Pss" => {
                out.pss_bytes = bytes;
                pss = true;
            }
            "Private_Clean" | "Private_Dirty" => out.private_bytes += bytes,
            "Swap" => out.swap_bytes = bytes,
            _ => {}
        }
    }
    pss.then_some(out)
}

/// Reads the rollup of one process.
pub fn read_smaps_rollup(pid: u32) -> Option<SmapsRollup> {
    parse_smaps_rollup(&fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROLLUP: &str = "\
55d0c0a00000-7ffd5a1f2000 ---p 00000000 00:00 0                          [rollup]
Rss:              123456 kB
Pss:               80000 kB
Pss_Dirty:         40000 kB
Pss_Anon:          30000 kB
Pss_File:          45000 kB
Pss_Shmem:          5000 kB
Shared_Clean:      50000 kB
Shared_Dirty:       1000 kB
Private_Clean:     20000 kB
Private_Dirty:     52456 kB
Referenced:       120000 kB
Anonymous:         70000 kB
KSM:                   0 kB
LazyFree:              0 kB
AnonHugePages:      2048 kB
ShmemPmdMapped:        0 kB
FilePmdMapped:         0 kB
Shared_Hugetlb:        0 kB
Private_Hugetlb:       0 kB
Swap:               4096 kB
SwapPss:            2048 kB
Locked:                0 kB
";

    #[test]
    fn parses_a_rollup() {
        assert_eq!(
            parse_smaps_rollup(ROLLUP),
            Some(SmapsRollup {
                pss_bytes: 80000 * 1024,
                private_bytes: 72456 * 1024,
                swap_bytes: 4096 * 1024,
            })
        );
        assert_eq!(parse_smaps_rollup(""), None);
        assert_eq!(parse_smaps_rollup("Rss: 5 kB\n"), None);
    }

    #[test]
    fn adds_rollups() {
        let a = SmapsRollup {
            pss_bytes: 1,
            private_bytes: 2,
            swap_bytes: 3,
        };
        assert_eq!(
            a.add(a),
            SmapsRollup {
                pss_bytes: 2,
                private_bytes: 4,
                swap_bytes: 6,
            }
        );
    }

    #[test]
    fn reads_own_rollup() {
        let r = read_smaps_rollup(std::process::id()).unwrap();
        assert!(r.pss_bytes > 0 && r.private_bytes > 0);
        assert_eq!(read_smaps_rollup(u32::MAX), None);
    }
}
