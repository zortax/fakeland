//! Commit calls of the session: wait for the next commit of a window tree, built on a per-commit
//! hook and a deadline task.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fakeland_scenario::WindowRef;
use fakeland_types::{CommitId, Nanos};

use super::{Session, SessionError};
use crate::display::TimerId;
use crate::frames::{CommitHookId, CommitPhase};
use crate::shell::WindowMatcher;
use crate::util::block_on;

impl Session {
    /// Waits for the next commit of any surface in the tree of a matching window. With
    /// `applied`, the wait ends when the content is applied at its latch.
    pub async fn wait_commit(
        &self,
        matcher: WindowRef,
        applied: bool,
        timeout: Nanos,
    ) -> Result<CommitId, SessionError> {
        let matcher = WindowMatcher::new(&matcher)?;
        let phase = if applied {
            CommitPhase::Applied
        } else {
            CommitPhase::Committed
        };
        self.inner
            .call_with(move |state, reply| {
                let Some(window) = matcher.select(&state.window_infos()).map(|w| w.id) else {
                    let _ = reply.send(Err(SessionError::NoWindow));
                    return;
                };
                let reply = Rc::new(RefCell::new(Some(reply)));
                let timer: Rc<Cell<Option<TimerId>>> = Rc::new(Cell::new(None));
                let hook_id: Rc<Cell<Option<CommitHookId>>> = Rc::new(Cell::new(None));
                let (hook_reply, hook_timer, own_id) =
                    (reply.clone(), timer.clone(), hook_id.clone());
                let id = state.add_commit_hook(move |state, event| {
                    let in_tree = state.windows.by_surface(event.id) == Some(window)
                        || state.window_of_surface(event.surface) == Some(window);
                    if event.phase != phase || !in_tree {
                        return;
                    }
                    let Some(&commit) = event.commits.last() else {
                        return;
                    };
                    if let Some(r) = hook_reply.borrow_mut().take() {
                        let _ = r.send(Ok(commit));
                    }
                    if let Some(t) = hook_timer.take() {
                        state.cancel_timer(t);
                    }
                    if let Some(h) = own_id.take() {
                        state.remove_commit_hook(h);
                    }
                });
                hook_id.set(Some(id));
                let deadline = state.schedule_in(timeout, move |state| {
                    if let Some(r) = reply.borrow_mut().take() {
                        if let Some(h) = hook_id.take() {
                            state.remove_commit_hook(h);
                        }
                        let _ = r.send(Err(SessionError::Timeout));
                    }
                });
                timer.set(Some(deadline));
            })
            .await?
    }

    /// Waits for the next commit on the calling thread.
    pub fn blocking_wait_commit(
        &self,
        matcher: WindowRef,
        applied: bool,
        timeout: Nanos,
    ) -> Result<CommitId, SessionError> {
        block_on(self.wait_commit(matcher, applied, timeout))
    }
}
