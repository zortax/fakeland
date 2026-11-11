//! The peer check of the control socket. Only processes of the user that runs the server may
//! connect; the check reads `SO_PEERCRED` of each accepted connection.

use std::os::unix::net::UnixStream;

use rustix::net::sockopt::socket_peercred;
use rustix::process::getuid;

/// Returns true when the peer of a connection runs as the same user as the server.
pub fn same_user(stream: &UnixStream) -> bool {
    match socket_peercred(stream) {
        Ok(cred) => cred.uid == getuid(),
        Err(e) => {
            tracing::warn!("cannot read the control peer credentials: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connection_from_this_process_is_the_same_user() {
        let (a, b) = UnixStream::pair().unwrap();
        assert!(same_user(&a));
        assert!(same_user(&b));
    }
}
