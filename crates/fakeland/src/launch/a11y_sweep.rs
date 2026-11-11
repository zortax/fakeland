//! Cleanup of accessibility buses that dead sessions left behind. The launcher of a session
//! whose host process died without cleanup ends, but the accessibility `dbus-daemon` it started
//! keeps running with its socket in the runtime directory `a11y-<pid>-<n>`. A new session ends
//! such daemons and removes their directories. A directory whose session process still runs
//! stays.

use std::path::Path;

use smithay::reexports::rustix::process::{Pid, Signal, kill_process};

/// The name prefix of accessibility runtime directories.
pub const PREFIX: &str = "a11y-";

/// Ends the daemons of stale accessibility runtime directories below `base` and removes them.
pub fn sweep(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(owner) = name.to_str().and_then(owner_pid) else {
            continue;
        };
        if Path::new(&format!("/proc/{owner}")).exists() {
            continue;
        }
        let dir = entry.path();
        for pid in users_of(&dir) {
            if let Some(pid) = Pid::from_raw(pid) {
                let _ = kill_process(pid, Signal::KILL);
            }
        }
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::debug!("cannot remove {}: {e}", dir.display());
        }
    }
}

/// Returns the session pid of a directory name `a11y-<pid>-<n>`.
fn owner_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(PREFIX)?;
    let (pid, n) = rest.split_once('-')?;
    n.parse::<u32>().ok()?;
    pid.parse().ok()
}

/// Returns the processes whose command line names a path below `dir`.
fn users_of(dir: &Path) -> Vec<i32> {
    let needle = format!("{}/", dir.to_string_lossy());
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    procs
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                raw.split(|b| *b == 0)
                    .any(|arg| String::from_utf8_lossy(arg).contains(&needle))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_names_carry_the_session_pid() {
        assert_eq!(owner_pid("a11y-1234-0"), Some(1234));
        assert_eq!(owner_pid("a11y-1234-x"), None);
        assert_eq!(owner_pid("a11y-x-0"), None);
        assert_eq!(owner_pid("wayland-1"), None);
    }

    #[test]
    fn stale_directories_go_and_live_ones_stay() {
        let base = std::env::temp_dir().join(format!("fl-sweep-{}", std::process::id()));
        let live = base.join(format!("{PREFIX}{}-0", std::process::id()));
        let stale = base.join(format!("{PREFIX}{}-0", u32::MAX - 7));
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&stale).unwrap();
        sweep(&base);
        assert!(live.exists());
        assert!(!stale.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
