//! The state of one surface on one output and the queries that schedule its next frame.

use std::collections::VecDeque;

use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::Entry;
use crate::Action;
use crate::callbacks::CallbackBook;

/// The commit-timing limit for updates that apply ahead of the latch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetLimit {
    /// The update presents at the refresh at this time.
    Frame(Timestamp),
    /// The update presents at the frame it applies for, so its target is its earliest apply time.
    AtApply,
}

/// The display state of one surface.
#[derive(Clone, Debug)]
pub(crate) struct SurfaceState {
    pub(super) id: SurfaceId,
    pub(super) queue: VecDeque<Entry>,
    /// Updates without content that applied ahead of the next latch.
    pub(super) open: Option<Entry>,
    /// The update that presents at the next refresh.
    pub(super) locked: Option<Entry>,
    /// Updates that the last latch discarded. Their buffers and callbacks wait for the refresh.
    pub(super) discards: Vec<Entry>,
    /// The commit whose buffer is on screen.
    pub(super) held: Option<CommitId>,
    /// An update that applied ahead of the latch set the barrier.
    pub(super) barrier_pending: bool,
    /// The refresh that clears the barrier.
    pub(super) barrier_clears_at: Option<u64>,
}

impl SurfaceState {
    pub(crate) fn new(id: SurfaceId) -> Self {
        Self {
            id,
            queue: VecDeque::new(),
            open: None,
            locked: None,
            discards: Vec::new(),
            held: None,
            barrier_pending: false,
            barrier_clears_at: None,
        }
    }

    pub(crate) fn push(&mut self, entry: Entry) {
        self.queue.push_back(entry);
    }

    /// Records the fence signal of a queued commit. Returns false for an unknown commit.
    pub(crate) fn mark_ready(&mut self, commit: CommitId, at: Timestamp) -> bool {
        match self.queue.iter_mut().find(|e| e.commit == commit) {
            Some(entry) => {
                entry.ready = Some(at);
                true
            }
            None => false,
        }
    }

    /// Returns the number of updates that have not reached the screen.
    pub(crate) fn unfinished(&self) -> usize {
        self.queue.len() + usize::from(self.open.is_some()) + usize::from(self.locked.is_some())
    }

    /// Returns the commit whose buffer is on screen.
    pub(crate) fn held(&self) -> Option<CommitId> {
        self.held
    }

    pub(super) fn barrier_set(&self) -> bool {
        self.barrier_pending || self.barrier_clears_at.is_some()
    }

    /// Returns the time at which the front update can apply ahead of the latch.
    pub(crate) fn apply_time(&self, limit: TargetLimit) -> Option<Timestamp> {
        let front = self.queue.front()?;
        if front.has_content || (front.fifo.wait_barrier && self.barrier_set()) {
            return None;
        }
        let avail = front.avail()?;
        match (limit, front.target) {
            (_, None) => Some(avail),
            (TargetLimit::Frame(frame), Some(t)) => (t <= frame).then_some(avail),
            (TargetLimit::AtApply, Some(t)) => Some(t.max(avail)),
        }
    }

    /// Returns the earliest frame time at which a latch takes a content update.
    pub(crate) fn content_need(&self) -> Option<Timestamp> {
        if let Some(locked) = self.locked.as_ref().filter(|e| e.has_content) {
            return locked.need();
        }
        let mut blocked = self.barrier_set();
        let mut need = Timestamp::ZERO;
        for e in &self.queue {
            if e.fifo.wait_barrier && blocked {
                return None;
            }
            need = need.max(e.need()?);
            blocked |= e.fifo.set_barrier;
            if e.has_content {
                return Some(need);
            }
        }
        None
    }

    /// Returns the earliest frame time at which a latch takes any update.
    pub(crate) fn any_need(&self) -> Option<Timestamp> {
        if let Some(e) = self.locked.as_ref().or(self.open.as_ref()) {
            return e.need();
        }
        let front = self.queue.front()?;
        if front.fifo.wait_barrier && self.barrier_set() {
            return None;
        }
        front.need()
    }

    /// Applies front updates without content ahead of the latch.
    pub(crate) fn apply_open(
        &mut self,
        at: Timestamp,
        limit: TargetLimit,
        seq: u64,
        book: &mut CallbackBook,
        out: &mut Vec<Action>,
    ) {
        while let Some(front) = self.queue.front() {
            let frame = match limit {
                TargetLimit::Frame(frame) => frame,
                TargetLimit::AtApply => at,
            };
            if front.has_content || !front.latchable(at, frame, self.barrier_set()) {
                break;
            }
            let Some(mut entry) = self.queue.pop_front() else {
                break;
            };
            self.barrier_pending |= entry.fifo.set_barrier;
            if let Some(old) = self.open.take() {
                out.push(Action::Discarded {
                    surface: self.id,
                    commit: old.commit,
                });
            }
            entry.latched_seq = Some(seq);
            out.push(Action::Latched {
                surface: self.id,
                commit: entry.commit,
                seq,
            });
            if entry.wants_callback {
                book.arm(self.id, entry.commit, at);
                entry.callback_armed = true;
            }
            self.open = Some(entry);
        }
    }

    /// Ends the surface. Every unfinished update is discarded and every buffer is released.
    pub(crate) fn remove(self, out: &mut Vec<Action>) {
        let surface = self.id;
        let unfinished = self
            .locked
            .into_iter()
            .chain(self.open)
            .chain(self.queue)
            .collect::<Vec<_>>();
        for e in &unfinished {
            out.push(Action::Discarded {
                surface,
                commit: e.commit,
            });
        }
        let buffers = self
            .held
            .into_iter()
            .chain(
                self.discards
                    .iter()
                    .filter(|e| e.has_content)
                    .map(|e| e.commit),
            )
            .chain(
                unfinished
                    .iter()
                    .filter(|e| e.has_content)
                    .map(|e| e.commit),
            );
        for commit in buffers {
            out.push(Action::Released { surface, commit });
        }
    }
}
