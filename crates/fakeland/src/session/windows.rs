//! Window calls of the session: listing, waits for windows and settled content, and window
//! operations.

use fakeland_scenario::WindowRef;
use fakeland_types::{Nanos, ProcessId};

use super::{Session, SessionError};
use crate::shell::{WindowInfo, WindowMatcher, WindowOp};
use crate::util::block_on;

impl Session {
    /// Returns every window, mapped ones first in mapping order.
    pub async fn windows(&self) -> Result<Vec<WindowInfo>, SessionError> {
        self.inner.call(|state| state.window_infos()).await
    }

    /// Returns every window on the calling thread.
    pub fn blocking_windows(&self) -> Result<Vec<WindowInfo>, SessionError> {
        block_on(self.windows())
    }

    /// Returns the windows of a launched process tree.
    pub(crate) async fn app_windows(
        &self,
        pid: ProcessId,
    ) -> Result<Vec<WindowInfo>, SessionError> {
        self.inner
            .call(move |state| {
                state
                    .window_infos()
                    .into_iter()
                    .filter(|w| w.app == Some(pid))
                    .collect()
            })
            .await
    }

    /// Waits for the first mapped window of a launched process tree.
    pub(crate) async fn wait_app_window(
        &self,
        pid: ProcessId,
        timeout: Nanos,
    ) -> Result<WindowInfo, SessionError> {
        self.inner
            .call_with(move |state, reply| {
                let deadline = state.now() + timeout;
                let mut reply = Some(reply);
                state.add_waiter(Some(deadline), move |state, timed_out| {
                    let found = state
                        .window_infos()
                        .into_iter()
                        .find(|w| w.is_mapped() && w.app == Some(pid));
                    let exited = state
                        .processes
                        .apps
                        .get(&pid.0)
                        .is_some_and(|a| a.exit().is_some());
                    let result = match (found, timed_out, exited) {
                        (Some(w), _, _) => Ok(w),
                        (None, _, true) => Err(SessionError::AppExited),
                        (None, true, false) => Err(SessionError::Timeout),
                        (None, false, false) => return false,
                    };
                    if let Some(r) = reply.take() {
                        let _ = r.send(result);
                    }
                    true
                });
            })
            .await?
    }

    /// Waits until a mapped window matches.
    pub async fn wait_window(
        &self,
        matcher: WindowRef,
        timeout: Nanos,
    ) -> Result<WindowInfo, SessionError> {
        let matcher = WindowMatcher::new(&matcher)?;
        self.inner
            .call_with(move |state, reply| {
                let deadline = state.now() + timeout;
                let mut reply = Some(reply);
                state.add_waiter(Some(deadline), move |state, timed_out| {
                    let found = matcher.select(&state.window_infos()).cloned();
                    let result = match (found, timed_out) {
                        (Some(w), _) => Ok(w),
                        (None, true) => Err(SessionError::Timeout),
                        (None, false) => return false,
                    };
                    if let Some(r) = reply.take() {
                        let _ = r.send(result);
                    }
                    true
                });
            })
            .await?
    }

    /// Waits until a mapped window matches, on the calling thread.
    pub fn blocking_wait_window(
        &self,
        matcher: WindowRef,
        timeout: Nanos,
    ) -> Result<WindowInfo, SessionError> {
        block_on(self.wait_window(matcher, timeout))
    }

    /// Waits until a matching window committed no damage for `quiet`. The quiet span starts no
    /// earlier than the call.
    pub async fn wait_settled(
        &self,
        matcher: WindowRef,
        quiet: Nanos,
        timeout: Nanos,
    ) -> Result<WindowInfo, SessionError> {
        let matcher = WindowMatcher::new(&matcher)?;
        self.inner
            .call_with(move |state, reply| {
                let started = state.now();
                let deadline = started + timeout;
                let mut reply = Some(reply);
                let mut armed = None;
                state.add_waiter(Some(deadline), move |state, timed_out| {
                    let now = state.now();
                    let window = matcher.select(&state.window_infos()).cloned();
                    let since = window
                        .as_ref()
                        .and_then(|w| state.quiet_since(w.id))
                        .map(|s| s.max(started));
                    let result = match (window, since) {
                        (Some(w), Some(since)) if now.since(since) >= quiet => Ok(w),
                        (_, since) if !timed_out => {
                            let wake = since.map(|s| s + quiet);
                            if let Some(at) = wake
                                && armed != Some(at)
                            {
                                armed = Some(at);
                                state.schedule_at(at, |_| {});
                            }
                            return false;
                        }
                        (None, _) => Err(SessionError::NoWindow),
                        _ => Err(SessionError::Timeout),
                    };
                    if let Some(r) = reply.take() {
                        let _ = r.send(result);
                    }
                    true
                });
            })
            .await?
    }

    /// Waits for settled content on the calling thread.
    pub fn blocking_wait_settled(
        &self,
        matcher: WindowRef,
        quiet: Nanos,
        timeout: Nanos,
    ) -> Result<WindowInfo, SessionError> {
        block_on(self.wait_settled(matcher, quiet, timeout))
    }

    /// Runs a window operation on the first matching window.
    pub async fn window_op(&self, matcher: WindowRef, op: WindowOp) -> Result<(), SessionError> {
        self.inner
            .call(move |state| {
                let id = state
                    .resolve_window(&matcher)?
                    .ok_or(SessionError::NoWindow)?;
                state.window_op(id, op)?;
                Ok(())
            })
            .await?
    }

    /// Runs a window operation on the calling thread.
    pub fn blocking_window_op(&self, matcher: WindowRef, op: WindowOp) -> Result<(), SessionError> {
        block_on(self.window_op(matcher, op))
    }
}
