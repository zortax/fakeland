//! Frame calls of the session: wait for presented frames of a window, built on an in-thread
//! observer and a deadline task.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fakeland_scenario::WindowRef;
use fakeland_trace::Event;
use fakeland_types::{Nanos, Timestamp};

use super::{Session, SessionError};
use crate::display::TimerId;
use crate::shell::WindowMatcher;
use crate::util::block_on;
use crate::waits::Observe;

impl Session {
    /// Waits until `n` frames of a matching window present after the call. Returns the
    /// presentation time of the last one.
    pub async fn wait_frames(
        &self,
        matcher: WindowRef,
        n: u32,
        timeout: Nanos,
    ) -> Result<Timestamp, SessionError> {
        let matcher = WindowMatcher::new(&matcher)?;
        self.inner
            .call_with(move |state, reply| {
                let Some(window) = matcher.select(&state.window_infos()).map(|w| w.id) else {
                    let _ = reply.send(Err(SessionError::NoWindow));
                    return;
                };
                if n == 0 {
                    let _ = reply.send(Ok(state.now()));
                    return;
                }
                let reply = Rc::new(RefCell::new(Some(reply)));
                let timer: Rc<Cell<Option<TimerId>>> = Rc::new(Cell::new(None));
                let mut left = n;
                let observer_reply = reply.clone();
                let observer_timer = timer.clone();
                let observer = state.observe(move |state, record| {
                    let Event::Presented { surface, time, .. } = &record.event else {
                        return Observe::Keep;
                    };
                    let in_window = state
                        .surfaces
                        .get(*surface)
                        .map(|s| s.wl.clone())
                        .and_then(|wl| state.window_of_surface(&wl))
                        == Some(window);
                    if !in_window {
                        return Observe::Keep;
                    }
                    left -= 1;
                    if left > 0 {
                        return Observe::Keep;
                    }
                    if let Some(r) = observer_reply.borrow_mut().take() {
                        let _ = r.send(Ok(*time));
                    }
                    if let Some(t) = observer_timer.take() {
                        state.cancel_timer(t);
                    }
                    Observe::Remove
                });
                let id = state.schedule_in(timeout, move |state| {
                    if let Some(r) = reply.borrow_mut().take() {
                        state.unobserve(observer);
                        let _ = r.send(Err(SessionError::Timeout));
                    }
                });
                timer.set(Some(id));
            })
            .await?
    }

    /// Waits for presented frames on the calling thread.
    pub fn blocking_wait_frames(
        &self,
        matcher: WindowRef,
        n: u32,
        timeout: Nanos,
    ) -> Result<Timestamp, SessionError> {
        block_on(self.wait_frames(matcher, n, timeout))
    }
}
