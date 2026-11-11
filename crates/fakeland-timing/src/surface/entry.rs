//! One queued content update and the latch rule for it.

use fakeland_types::{CommitId, Timestamp};

use crate::{FifoFlags, Readiness, Update};

/// A content update in a surface queue.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) commit: CommitId,
    pub(crate) has_content: bool,
    pub(crate) wants_callback: bool,
    pub(crate) fifo: FifoFlags,
    pub(crate) target: Option<Timestamp>,
    pub(crate) submitted: Timestamp,
    pub(crate) ready: Option<Timestamp>,
    pub(crate) latched_seq: Option<u64>,
    pub(crate) callback_armed: bool,
}

impl Entry {
    pub(crate) fn new(update: &Update, submitted: Timestamp) -> Self {
        Self {
            commit: update.commit,
            has_content: update.has_content,
            wants_callback: update.wants_callback,
            fifo: update.fifo,
            target: update.target,
            submitted,
            ready: match update.readiness {
                Readiness::Ready(at) => Some(at),
                Readiness::Pending => None,
            },
            latched_seq: None,
            callback_armed: false,
        }
    }

    /// Returns the time from which the update can latch, or `None` while its fence is pending.
    pub(crate) fn avail(&self) -> Option<Timestamp> {
        self.ready.map(|r| r.max(self.submitted))
    }

    /// Returns the earliest frame time that can present the update when no barrier blocks it.
    pub(crate) fn need(&self) -> Option<Timestamp> {
        let avail = self.avail()?;
        Some(self.target.map_or(avail, |t| t.max(avail)))
    }

    /// Returns true when the update can latch at `latch` for a frame at `frame`.
    pub(crate) fn latchable(&self, latch: Timestamp, frame: Timestamp, barrier_set: bool) -> bool {
        let ready = self.avail().is_some_and(|a| a <= latch);
        let on_time = self.target.is_none_or(|t| t <= frame);
        let unblocked = !(self.fifo.wait_barrier && barrier_set);
        ready && on_time && unblocked
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::SurfaceId;

    use super::*;

    fn entry(update: Update) -> Entry {
        Entry::new(&update, Timestamp(100))
    }

    #[test]
    fn readiness_never_precedes_the_submission() {
        let e = entry(Update::new(SurfaceId(1), CommitId(1)).ready_at(Timestamp(50)));
        assert_eq!(e.avail(), Some(Timestamp(100)));
        let e = entry(Update::new(SurfaceId(1), CommitId(1)).ready_at(Timestamp(150)));
        assert_eq!(e.avail(), Some(Timestamp(150)));
        let e = entry(Update::new(SurfaceId(1), CommitId(1)).pending());
        assert_eq!(e.avail(), None);
        assert!(!e.latchable(Timestamp(1_000), Timestamp(1_000), false));
    }

    #[test]
    fn latch_rule() {
        let base = Update::new(SurfaceId(1), CommitId(1));
        let e = entry(base.target(Timestamp(500)));
        assert!(!e.latchable(Timestamp(200), Timestamp(400), false));
        assert!(e.latchable(Timestamp(200), Timestamp(500), false));
        assert_eq!(e.need(), Some(Timestamp(500)));
        let e = entry(base.fifo(false, true));
        assert!(!e.latchable(Timestamp(200), Timestamp(300), true));
        assert!(e.latchable(Timestamp(200), Timestamp(300), false));
        assert!(!e.latchable(Timestamp(99), Timestamp(300), false));
    }
}
