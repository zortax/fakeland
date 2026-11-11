//! The parent-death guard of children. A child starts through `setpriv --pdeathsig KILL`, which
//! sets the parent-death signal and then executes the program under the same pid. The kernel
//! kills the child when the thread that started it ends, so a child also dies when the host
//! process dies without cleanup. Children start on the compositor thread, which lives as long as
//! the session. Without `setpriv` the child starts directly.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The guard program.
const SETPRIV: &str = "setpriv";

/// Returns a command that starts `program` under the parent-death guard. `path` is the `PATH` of
/// the child. A program that is missing fails here with `NotFound`, as a direct spawn would.
pub fn guarded(program: &str, path: Option<&OsStr>) -> std::io::Result<Command> {
    let Some(setpriv) = setpriv() else {
        return Ok(Command::new(program));
    };
    let resolved = resolve(program, path).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program}: no such executable"),
        )
    })?;
    let mut cmd = Command::new(setpriv);
    cmd.args(["--pdeathsig", "KILL", "--"]).arg(resolved);
    Ok(cmd)
}

/// Returns the path of `setpriv` on the `PATH` of this process, once.
fn setpriv() -> Option<&'static Path> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| resolve(SETPRIV, std::env::var_os("PATH").as_deref()))
        .as_deref()
}

/// Resolves a program as `execvp` does: a name with a slash is a path, other names search `path`.
fn resolve(program: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if program.contains('/') {
        let p = PathBuf::from(program);
        return executable(&p).then_some(p);
    }
    std::env::split_paths(path?)
        .map(|dir| dir.join(program))
        .find(|p| executable(p))
}

fn executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_resolve_like_execvp() {
        let path = std::env::var_os("PATH");
        assert!(resolve("sh", path.as_deref()).is_some());
        assert!(resolve("/bin/sh", None).is_some());
        assert!(resolve("no-such-program-xyz", path.as_deref()).is_none());
        assert!(resolve("/nonexistent/program", path.as_deref()).is_none());
        assert!(resolve("sh", None).is_none());
    }

    #[test]
    fn missing_programs_fail_before_spawn() {
        let path = std::env::var_os("PATH");
        let err = guarded("/nonexistent/program", path.as_deref()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        let out = guarded("sh", path.as_deref())
            .unwrap()
            .args(["-c", "echo $$"])
            .output()
            .unwrap();
        assert!(out.status.success());
    }
}
