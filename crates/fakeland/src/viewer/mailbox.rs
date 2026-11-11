//! A channel whose receiver sleeps in `poll`: each send writes an eventfd, so the viewer thread
//! waits for the host connection and for compositor messages at once.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use smithay::reexports::rustix::event::{EventfdFlags, eventfd};
use smithay::reexports::rustix::io;

/// The sending side.
#[derive(Debug)]
pub struct MailSender<T> {
    tx: Sender<T>,
    wake: Arc<OwnedFd>,
}

impl<T> Clone for MailSender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            wake: self.wake.clone(),
        }
    }
}

/// The receiving side.
#[derive(Debug)]
pub struct MailReceiver<T> {
    rx: Receiver<T>,
    wake: Arc<OwnedFd>,
}

/// Builds a mailbox.
pub fn mailbox<T>() -> std::io::Result<(MailSender<T>, MailReceiver<T>)> {
    let fd = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
    let wake = Arc::new(fd);
    let (tx, rx) = mpsc::channel();
    Ok((
        MailSender {
            tx,
            wake: wake.clone(),
        },
        MailReceiver { rx, wake },
    ))
}

impl<T> MailSender<T> {
    /// Sends a message and wakes the receiver. Returns false when the receiver is gone.
    pub fn send(&self, msg: T) -> bool {
        if self.tx.send(msg).is_err() {
            return false;
        }
        let _ = io::write(&*self.wake, &1u64.to_ne_bytes());
        true
    }
}

impl<T> MailReceiver<T> {
    /// Returns the fd that turns readable when mail arrives.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }

    /// Resets the wake fd and returns every queued message. The second value is true when every
    /// sender is gone.
    pub fn drain(&self) -> (Vec<T>, bool) {
        let mut buf = [0u8; 8];
        let _ = io::read(&*self.wake, &mut buf);
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(m) => out.push(m),
                Err(TryRecvError::Empty) => return (out, false),
                Err(TryRecvError::Disconnected) => return (out, true),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay::reexports::rustix::event::{PollFd, PollFlags, poll};
    use smithay::reexports::rustix::time::Timespec;

    use super::*;

    #[test]
    fn sends_wake_the_fd_and_drain_resets_it() {
        let (tx, rx) = mailbox::<u32>().unwrap();
        let ready = |rx: &MailReceiver<u32>| {
            let fd = rx.fd();
            let mut fds = [PollFd::new(&fd, PollFlags::IN)];
            let zero = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            poll(&mut fds, Some(&zero)).unwrap() > 0
        };
        assert!(!ready(&rx));
        assert!(tx.send(1));
        assert!(tx.clone().send(2));
        assert!(ready(&rx));
        assert_eq!(rx.drain(), (vec![1, 2], false));
        assert!(!ready(&rx));
        drop(tx);
        assert_eq!(rx.drain(), (vec![], true));
    }
}
