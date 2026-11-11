//! Socket paths and binding. Control sockets live in `$XDG_RUNTIME_DIR/fakeland/`, a directory
//! with mode 0700, and each socket has mode 0600, so only the owner can connect.

use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use thiserror::Error;

/// The environment variable that names a control socket for clients.
pub const SOCKET_ENV: &str = "FAKELAND_SOCKET";

/// A socket that cannot be bound.
#[derive(Debug, Error)]
pub enum BindError {
    /// No runtime directory is known.
    #[error("XDG_RUNTIME_DIR is not set; pass a socket path")]
    NoRuntimeDir,
    /// Another server listens on the path.
    #[error("{0} is in use by a running server")]
    InUse(PathBuf),
    /// A file operation failed.
    #[error("{path}: {error}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
}

/// Returns the directory of control sockets, `$XDG_RUNTIME_DIR/fakeland`.
pub fn socket_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join("fakeland"))
}

/// Returns the default socket path of a session: `$XDG_RUNTIME_DIR/fakeland/<name>.sock`.
pub fn default_socket_path(name: &str) -> Result<PathBuf, BindError> {
    Ok(socket_dir()
        .ok_or(BindError::NoRuntimeDir)?
        .join(format!("{name}.sock")))
}

/// Returns the sockets in the socket directory that accept a connection, newest first.
pub fn discover_sockets() -> Vec<PathBuf> {
    let Some(dir) = socket_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sock"))
        .filter(|p| UnixStream::connect(p).is_ok())
        .map(|p| {
            let modified = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (modified, p)
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Binds a listener at a path. The parent directory is created with mode 0700, a stale socket
/// file is removed, and the socket gets mode 0600.
pub fn bind(path: &Path) -> Result<UnixListener, BindError> {
    let io = |error| BindError::Io {
        path: path.to_path_buf(),
        error,
    };
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(io)?;
    }
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(BindError::InUse(path.to_path_buf()));
        }
        std::fs::remove_file(path).map_err(io)?;
    }
    let listener = UnixListener::bind(path).map_err(io)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(io)?;
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn binds_with_owner_only_modes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ctl/one.sock");
        let listener = bind(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert!(matches!(bind(&path), Err(BindError::InUse(_))));
        drop(listener);
        let _again = bind(&path).unwrap();
    }
}
