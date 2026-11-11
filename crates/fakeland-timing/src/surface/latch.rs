//! The latch walk: take every update that is ready, on time and unblocked, in queue order.

use fakeland_types::Timestamp;

use super::SurfaceState;
use crate::Action;

impl SurfaceState {
    /// Latches updates for the refresh `seq` at `frame`. Updates must be ready by `latch`. The
    /// last taken update presents at the refresh, and earlier taken updates are discarded.
    pub(crate) fn latch(
        &mut self,
        seq: u64,
        latch: Timestamp,
        frame: Timestamp,
        out: &mut Vec<Action>,
    ) {
        let mut taken: Vec<_> = self.locked.take().into_iter().collect();
        taken.extend(self.open.take());
        let mut blocked = self.barrier_set();
        let mut set_now = false;
        while let Some(front) = self.queue.front() {
            if !front.latchable(latch, frame, blocked) {
                break;
            }
            let Some(entry) = self.queue.pop_front() else {
                break;
            };
            if entry.fifo.set_barrier {
                blocked = true;
                set_now = true;
            }
            taken.push(entry);
        }
        let Some(mut last) = taken.pop() else {
            return;
        };
        if set_now || self.barrier_set() {
            self.barrier_pending = false;
            self.barrier_clears_at = Some(seq);
        }
        for entry in taken {
            out.push(Action::Discarded {
                surface: self.id,
                commit: entry.commit,
            });
            self.discards.push(entry);
        }
        if last.latched_seq != Some(seq) {
            last.latched_seq = Some(seq);
            out.push(Action::Latched {
                surface: self.id,
                commit: last.commit,
                seq,
            });
        }
        self.locked = Some(last);
    }
}
