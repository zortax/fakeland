//! Spawning a child on the compositor thread: its environment, its own process group, threads
//! that copy stdout and stderr into ring buffers, and a waiter thread that reports the exit.

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Stdio};
use std::sync::Arc;

use fakeland_trace::Event;
use fakeland_types::ProcessId;

use super::deathsig::guarded;
use super::{AppShared, ChildEnv, ExitInfo, ExitMsg, LaunchError, LaunchSpec, RingBuffer};
use crate::server::State;

impl State {
    /// Starts a child process in the session.
    pub fn spawn_app(&mut self, spec: &LaunchSpec) -> Result<Arc<AppShared>, LaunchError> {
        if spec.program.is_empty() {
            return Err(LaunchError::EmptyProgram);
        }
        if let Some(o) = spec.output
            && self.outputs.get(o).is_none()
        {
            return Err(LaunchError::NoOutput(o));
        }
        let dbus = self.processes.dbus.as_ref().map(|d| d.address.clone());
        let env = ChildEnv {
            socket: &self.socket_name,
            dbus_address: dbus.as_deref(),
            session_env: &self.config.env,
            launch_env: &spec.env,
            accessibility: self.config.accessibility,
        }
        .build(std::env::vars());
        let spawn_error = |source| LaunchError::Spawn {
            program: spec.program.clone(),
            source,
        };
        let path = env.get("PATH").map(std::ffi::OsStr::new);
        let mut cmd = guarded(&spec.program, path).map_err(spawn_error)?;
        cmd.args(&spec.args)
            .env_clear()
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if let Some(dir) = &spec.cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(spawn_error)?;
        let pid = ProcessId(child.id());
        let command = spec.command_line();
        let shared = Arc::new(AppShared::new(
            pid,
            command.clone(),
            self.processes.log_capacity,
        ));
        if let Some(out) = child.stdout.take() {
            copy_stream(out, shared.clone(), |s| &s.stdout);
        }
        if let Some(err) = child.stderr.take() {
            copy_stream(err, shared.clone(), |s| &s.stderr);
        }
        spawn_waiter(child, shared.clone(), self.processes.exits.clone());
        self.processes.apps.insert(pid.0, shared.clone());
        if let Some(o) = spec.output.or_else(|| self.outputs.primary()) {
            self.processes.outputs.insert(pid.0, o);
        }
        self.emit(Event::ProcessSpawned { pid, command });
        Ok(shared)
    }

    /// Records a child exit.
    pub fn process_exited(&mut self, msg: ExitMsg) {
        self.emit(Event::ProcessExited {
            pid: msg.pid,
            code: msg.info.code,
            signal: msg.info.signal,
        });
    }
}

fn copy_stream<R>(
    mut stream: R,
    shared: Arc<AppShared>,
    pick: fn(&AppShared) -> &std::sync::Mutex<RingBuffer>,
) where
    R: Read + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name(format!("child-{}-log", shared.pid.0))
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut ring) = pick(&shared).lock() {
                            ring.write(&buf[..n]);
                        }
                    }
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("cannot start a log reader: {e}");
    }
}

fn spawn_waiter(
    mut child: Child,
    shared: Arc<AppShared>,
    exits: smithay::reexports::calloop::channel::Sender<ExitMsg>,
) {
    let pid = shared.pid;
    let spawned = std::thread::Builder::new()
        .name(format!("child-{}-wait", pid.0))
        .spawn(move || {
            let info = match child.wait() {
                Ok(status) => ExitInfo {
                    code: status.code(),
                    signal: status.signal(),
                },
                Err(_) => ExitInfo {
                    code: None,
                    signal: None,
                },
            };
            shared.set_exit(info);
            let _ = exits.send(ExitMsg { pid, info });
        });
    if let Err(e) = spawned {
        tracing::warn!("cannot start a child waiter: {e}");
    }
}
