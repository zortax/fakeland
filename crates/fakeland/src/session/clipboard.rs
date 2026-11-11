//! Clipboard calls of the session: set a text selection that the compositor serves, and read the
//! selection of the focused client through a pipe.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::sync::Arc;

use fakeland_types::{Nanos, Timestamp};
use futures_channel::oneshot;
use smithay::reexports::rustix::event::{PollFd, PollFlags, poll};
use smithay::reexports::rustix::pipe::{PipeFlags, pipe_with};
use smithay::reexports::rustix::time::Timespec;
use smithay::wayland::selection::data_device::request_data_device_client_selection;

use super::{Session, SessionError};
use crate::selection::TEXT_MIME;
use crate::util::block_on;
impl Session {
    /// Sets the clipboard to a text that the compositor serves.
    pub async fn set_clipboard(
        &self,
        mime: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<(), SessionError> {
        let mime = mime.into();
        let bytes: Arc<[u8]> = Arc::from(text.into().into_bytes().into_boxed_slice());
        self.inner
            .call(move |state| state.offer_selection(&mime, bytes))
            .await
    }

    /// Sets the clipboard on the calling thread.
    pub fn blocking_set_clipboard(
        &self,
        mime: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<(), SessionError> {
        block_on(self.set_clipboard(mime, text))
    }

    /// Sets the clipboard to UTF-8 text.
    pub async fn set_clipboard_text(&self, text: impl Into<String>) -> Result<(), SessionError> {
        self.set_clipboard(TEXT_MIME, text).await
    }

    /// Sets the clipboard to UTF-8 text on the calling thread.
    pub fn blocking_set_clipboard_text(&self, text: impl Into<String>) -> Result<(), SessionError> {
        block_on(self.set_clipboard_text(text))
    }

    /// Reads the client selection as text. The owning client has `timeout` to write it.
    pub async fn clipboard_text(&self, timeout: Nanos) -> Result<String, SessionError> {
        let rx =
            self.inner
                .call(
                    move |state| -> Result<
                        oneshot::Receiver<Result<String, SessionError>>,
                        SessionError,
                    > {
                        if let Some(own) = &state.selection {
                            let (tx, rx) = oneshot::channel();
                            let _ = tx.send(Ok(String::from_utf8_lossy(own).into_owned()));
                            return Ok(rx);
                        }
                        let (read, write) = pipe_with(PipeFlags::CLOEXEC)
                            .map_err(|e| SessionError::Clipboard(e.to_string()))?;
                        request_data_device_client_selection(
                            &state.seat,
                            TEXT_MIME.to_string(),
                            write,
                        )
                        .map_err(|e| SessionError::Clipboard(e.to_string()))?;
                        state.flush_clients();
                        let deadline = state.now() + timeout;
                        let clock = state.clock.clone();
                        let (tx, rx) = oneshot::channel();
                        std::thread::Builder::new()
                            .name("selection-read".into())
                            .spawn(move || {
                                let _ = tx.send(read_until(read, deadline, &*clock));
                            })
                            .map_err(|e| SessionError::Clipboard(e.to_string()))?;
                        Ok(rx)
                    },
                )
                .await??;
        rx.await.map_err(|_| SessionError::Closed)?
    }

    /// Reads the client selection on the calling thread.
    pub fn blocking_clipboard_text(&self, timeout: Nanos) -> Result<String, SessionError> {
        block_on(self.clipboard_text(timeout))
    }

    /// Sets the clipboard to text; the same as `set_clipboard`.
    pub async fn clipboard_set(
        &self,
        mime: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.set_clipboard(mime, text).await
    }

    /// Sets the clipboard to text on the calling thread.
    pub fn blocking_clipboard_set(
        &self,
        mime: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<(), SessionError> {
        block_on(self.clipboard_set(mime, text))
    }

    /// Reads the clipboard as text; the same as `clipboard_text`.
    pub async fn clipboard_get(&self, timeout: Nanos) -> Result<String, SessionError> {
        self.clipboard_text(timeout).await
    }

    /// Reads the clipboard as text on the calling thread.
    pub fn blocking_clipboard_get(&self, timeout: Nanos) -> Result<String, SessionError> {
        block_on(self.clipboard_get(timeout))
    }
}

fn read_until(
    fd: OwnedFd,
    deadline: Timestamp,
    clock: &dyn fakeland_types::Clock,
) -> Result<String, SessionError> {
    let mut file = std::fs::File::from(fd);
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let left = deadline.since(clock.now());
        if left <= Nanos::ZERO {
            return Err(SessionError::Timeout);
        }
        let (secs, nanos) = (
            left.as_nanos() / 1_000_000_000,
            left.as_nanos() % 1_000_000_000,
        );
        let ts = Timespec {
            tv_sec: secs,
            tv_nsec: nanos,
        };
        let mut fds = [PollFd::new(&file, PollFlags::IN | PollFlags::HUP)];
        poll(&mut fds, Some(&ts)).map_err(|e| SessionError::Clipboard(e.to_string()))?;
        if fds[0].revents().is_empty() {
            continue;
        }
        match file.read(&mut buf) {
            Ok(0) => return Ok(String::from_utf8_lossy(&out).into_owned()),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(SessionError::Clipboard(e.to_string())),
        }
    }
}
