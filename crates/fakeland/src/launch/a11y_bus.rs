//! The accessibility bus of a private session bus: `at-spi-bus-launcher` in its own process
//! group, with its socket in a private runtime directory, so sessions never share an
//! accessibility bus. The launcher owns `org.a11y.Bus` on the session bus, runs the accessibility
//! `dbus-daemon`, and reports `IsEnabled`. The AT-SPI registry starts through bus activation on
//! the accessibility bus.

use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use fakeland_a11y::{A11y, DEFAULT_CALL_TIMEOUT, block_on, bus_address};
use smithay::reexports::rustix::process::{Pid, Signal, kill_process_group};
use thiserror::Error;

use super::deathsig::guarded;
use crate::config::SessionConfig;

/// The launcher binary.
pub const LAUNCHER: &str = "/usr/lib/at-spi-bus-launcher";

/// How often the start polls for the bus.
const POLL: Duration = Duration::from_millis(20);

/// How many polls the start waits for the bus.
const POLLS: u32 = 250;

/// Variables the launcher keeps from the caller.
const KEPT: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "XDG_DATA_DIRS",
];

/// Numbers runtime directories within one process.
static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

/// An accessibility bus that fails to start.
#[derive(Debug, Error)]
pub enum A11yBusError {
    /// The runtime directory does not exist.
    #[error("no runtime directory for the accessibility bus: {0}")]
    RuntimeDir(String),
    /// The launcher does not start.
    #[error("cannot start {LAUNCHER}: {0}")]
    Spawn(std::io::Error),
    /// The launcher ends before the bus is up.
    #[error("{LAUNCHER} exited before the accessibility bus came up")]
    Exited,
    /// The bus does not come up in time.
    #[error("the accessibility bus did not come up: {0}")]
    Timeout(String),
    /// The registry does not start.
    #[error("the AT-SPI registry did not start: {0}")]
    Registry(String),
}

/// A running accessibility bus.
#[derive(Debug)]
pub struct A11yBus {
    child: Child,
    dir: PathBuf,
    /// The accessibility bus address.
    pub address: String,
}

impl A11yBus {
    /// Starts the launcher on a session bus and waits until the bus and the registry run.
    pub fn start(session_bus: &str, screen_reader: bool) -> Result<Self, A11yBusError> {
        let mut cmd =
            guarded(LAUNCHER, std::env::var_os("PATH").as_deref()).map_err(A11yBusError::Spawn)?;
        let dir = runtime_dir()?;
        cmd.args([
            "--launch-immediately",
            "--a11y=1",
            if screen_reader {
                "--screen-reader=1"
            } else {
                "--screen-reader=0"
            },
        ])
        .env_clear()
        .envs(
            KEPT.iter()
                .filter_map(|k| Some((*k, std::env::var(k).ok()?))),
        )
        .env("XDG_RUNTIME_DIR", &dir)
        .env("DBUS_SESSION_BUS_ADDRESS", session_bus)
        .env("GSETTINGS_BACKEND", "memory")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(A11yBusError::Spawn(e));
            }
        };
        let mut bus = Self {
            child,
            dir,
            address: String::new(),
        };
        bus.address = bus.wait_for_address(session_bus)?;
        let a11y = block_on(A11y::connect_bus(&bus.address, DEFAULT_CALL_TIMEOUT))
            .map_err(|e| A11yBusError::Registry(e.to_string()))?;
        a11y.blocking_start_registry()
            .map_err(|e| A11yBusError::Registry(e.to_string()))?;
        Ok(bus)
    }

    fn wait_for_address(&mut self, session_bus: &str) -> Result<String, A11yBusError> {
        let mut last = String::new();
        for _ in 0..POLLS {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return Err(A11yBusError::Exited);
            }
            match block_on(bus_address(session_bus)) {
                Ok(address) => return Ok(address),
                Err(e) => last = e.to_string(),
            }
            std::thread::sleep(POLL);
        }
        Err(A11yBusError::Timeout(last))
    }
}

impl Drop for A11yBus {
    fn drop(&mut self) {
        if let Some(pid) = Pid::from_raw(self.child.id() as i32) {
            let _ = kill_process_group(pid, Signal::TERM);
            let mut ended = false;
            for _ in 0..50 {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    ended = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if !ended {
                let _ = kill_process_group(pid, Signal::KILL);
                let _ = self.child.wait();
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Starts the accessibility bus of a session with a private bus and accessibility on. A bus that
/// fails to start leaves the session without accessibility and logs a warning.
pub fn start_a11y_bus(config: &SessionConfig, session_bus: Option<&str>) -> Option<A11yBus> {
    let bus = session_bus.filter(|_| config.accessibility)?;
    match A11yBus::start(bus, config.screen_reader) {
        Ok(b) => Some(b),
        Err(e) => {
            tracing::warn!("accessibility is off: {e}");
            None
        }
    }
}

/// Creates a private runtime directory for the bus socket below `$XDG_RUNTIME_DIR`, after the
/// sweep of stale directories.
fn runtime_dir() -> Result<PathBuf, A11yBusError> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    super::a11y_sweep::sweep(&base);
    let pid = std::process::id();
    for _ in 0..64 {
        let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("{}{pid}-{n}", super::a11y_sweep::PREFIX));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(A11yBusError::RuntimeDir(e.to_string())),
        }
    }
    Err(A11yBusError::RuntimeDir(format!(
        "no free directory name in {}",
        base.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_dirs_are_private_and_distinct() {
        let a = runtime_dir().unwrap();
        let b = runtime_dir().unwrap();
        assert_ne!(a, b);
        let mode = std::fs::metadata(&a).unwrap();
        assert!(mode.is_dir());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(mode.permissions().mode() & 0o777, 0o700);
        assert!(!a.to_string_lossy().to_lowercase().contains("fakeland"));
        std::fs::remove_dir(a).unwrap();
        std::fs::remove_dir(b).unwrap();
    }
}
