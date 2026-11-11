//! The connection to the host compositor: a display name or path, or `WAYLAND_DISPLAY` of this
//! process.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use wayland_client::Connection;

/// Returns the socket path of a display name. An absolute name is a path; another name lies in
/// `$XDG_RUNTIME_DIR`.
pub fn socket_path(display: &str, runtime_dir: Option<&str>) -> Result<PathBuf, String> {
    let path = PathBuf::from(display);
    if path.is_absolute() {
        return Ok(path);
    }
    let dir = runtime_dir.ok_or("XDG_RUNTIME_DIR is not set")?;
    Ok(PathBuf::from(dir).join(display))
}

/// Connects to the host display.
pub fn connect(display: Option<&str>) -> Result<Connection, String> {
    let name = match display {
        Some(d) => d.to_string(),
        None => std::env::var("WAYLAND_DISPLAY")
            .map_err(|_| "the viewer needs a host WAYLAND_DISPLAY".to_string())?,
    };
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok();
    let path = socket_path(&name, runtime.as_deref())?;
    let stream = UnixStream::connect(&path)
        .map_err(|e| format!("cannot connect to {}: {e}", path.display()))?;
    Connection::from_socket(stream).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_names_and_paths() {
        assert_eq!(
            socket_path("wayland-3", Some("/run/user/1000")).unwrap(),
            PathBuf::from("/run/user/1000/wayland-3")
        );
        assert_eq!(
            socket_path("/tmp/x/wayland-1", None).unwrap(),
            PathBuf::from("/tmp/x/wayland-1")
        );
        assert!(socket_path("wayland-1", None).is_err());
    }
}
