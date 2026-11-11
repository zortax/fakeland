//! Fence readiness of commits. An unsignaled explicit acquire point or implicit dmabuf fence gets
//! an event source; when it fires, the scheduler learns the ready time and the trace records it.

use std::io;

use fakeland_trace::Event;
use fakeland_types::CommitId;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::sync::Fence;
use smithay::reexports::calloop::{EventSource, Interest};

use super::Collected;
use crate::server::State;

impl State {
    /// Starts fence watches for a commit. Returns true when the content is ready at once.
    pub(super) fn watch_readiness(
        &mut self,
        commit: CommitId,
        collected: &Collected,
        dmabuf: Option<&Dmabuf>,
    ) -> bool {
        if let Some(point) = &collected.acquire {
            if point.is_signaled() {
                return true;
            }
            return match point.generate_blocker() {
                Ok((_blocker, source)) => !self.watch_fence(commit, source),
                Err(e) => {
                    tracing::warn!("cannot watch an acquire point: {e}");
                    true
                }
            };
        }
        if let Some(dmabuf) = dmabuf {
            return match dmabuf.generate_blocker(Interest::READ) {
                Ok((_blocker, source)) => !self.watch_fence(commit, source),
                Err(_) => true,
            };
        }
        true
    }

    /// Inserts a fence source. Returns true when the source is live.
    fn watch_fence<S>(&mut self, commit: CommitId, source: S) -> bool
    where
        S: EventSource<Event = (), Ret = io::Result<()>> + 'static,
    {
        self.loop_handle
            .insert_source(source, move |_, _, state| {
                state.commit_ready(commit);
                Ok(())
            })
            .map_err(|e| tracing::warn!("cannot watch a fence: {}", e.error))
            .is_ok()
    }

    /// Tells the scheduler that the content of a commit is ready now.
    pub fn commit_ready(&mut self, commit: CommitId) {
        let Some(p) = self.frames.pending.get(&commit) else {
            return;
        };
        let (surface, output) = (p.surface, p.output);
        let now = self.now();
        let marked = self
            .outputs
            .get_mut(output)
            .is_some_and(|o| o.scheduler.mark_ready(commit, now));
        if marked {
            self.emit(Event::Ready { surface, commit });
        }
    }
}
