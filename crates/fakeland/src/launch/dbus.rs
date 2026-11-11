//! The private D-Bus session bus: a `dbus-daemon` child in its own process group that prints its
//! address on stdout. The daemon starts with a minimal environment in `/`, so the services it
//! activates never inherit the host display. The session then sets the activation environment
//! to the child environment of the session.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use smithay::reexports::rustix::process::{Pid, Signal, kill_process_group};
use thiserror::Error;

use super::deathsig::guarded;

/// Variables the daemon keeps from the caller.
const KEPT: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "XDG_RUNTIME_DIR",
    "XDG_DATA_DIRS",
    "XDG_CONFIG_DIRS",
];

/// How long the daemon may take to print its address.
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// A private bus that fails to start.
#[derive(Debug, Error)]
pub enum DbusError {
    /// The daemon does not start.
    #[error("cannot start dbus-daemon: {0}")]
    Spawn(std::io::Error),
    /// The daemon prints no address.
    #[error("dbus-daemon printed no address")]
    NoAddress,
    /// The daemon refuses the activation environment.
    #[error("cannot set the activation environment: {0}")]
    Activation(String),
}

/// A running private bus.
#[derive(Debug)]
pub struct DbusDaemon {
    child: Child,
    /// The bus address for `DBUS_SESSION_BUS_ADDRESS`.
    pub address: String,
}

impl DbusDaemon {
    /// Starts a daemon with the session bus configuration.
    pub fn start() -> Result<Self, DbusError> {
        let mut child = guarded("dbus-daemon", std::env::var_os("PATH").as_deref())
            .map_err(DbusError::Spawn)?
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .env_clear()
            .envs(
                KEPT.iter()
                    .filter_map(|k| Some((*k, std::env::var(k).ok()?))),
            )
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(DbusError::Spawn)?;
        let stdout = child.stdout.take().ok_or(DbusError::NoAddress)?;
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("dbus-address".into())
            .spawn(move || {
                let mut line = String::new();
                let _ = BufReader::new(stdout).read_line(&mut line);
                let _ = tx.send(line);
            })
            .map_err(DbusError::Spawn)?;
        let address = match rx.recv_timeout(START_TIMEOUT) {
            Ok(line) if !line.trim().is_empty() => line.trim().to_string(),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DbusError::NoAddress);
            }
        };
        Ok(Self { child, address })
    }
}

impl DbusDaemon {
    /// Sets variables in the environment of every service the bus activates from now on.
    pub fn set_activation_env(&self, env: &BTreeMap<String, String>) -> Result<(), DbusError> {
        let vars: HashMap<&str, &str> = env
            .iter()
            .filter(|(k, _)| !k.starts_with("DBUS_STARTER_"))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let call = async {
            let conn = zbus::connection::Builder::address(self.address.as_str())?
                .method_timeout(START_TIMEOUT)
                .build()
                .await?;
            zbus::fdo::DBusProxy::new(&conn)
                .await?
                .update_activation_environment(vars)
                .await?;
            Ok::<(), zbus::Error>(())
        };
        crate::block_on(call).map_err(|e| DbusError::Activation(e.to_string()))
    }
}

impl Drop for DbusDaemon {
    fn drop(&mut self) {
        let Some(pid) = Pid::from_raw(self.child.id() as i32) else {
            return;
        };
        let _ = kill_process_group(pid, Signal::TERM);
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = kill_process_group(pid, Signal::KILL);
        let _ = self.child.wait();
    }
}
