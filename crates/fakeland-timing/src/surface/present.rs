//! Presentation of the latched update: feedback, buffer release, callbacks and barrier clearing.

use fakeland_types::{Nanos, Timestamp};

use super::SurfaceState;
use crate::callbacks::CallbackBook;
use crate::{Action, PresentFlags};

/// The feedback values of one refresh.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) seq: u64,
    pub(crate) time: Timestamp,
    pub(crate) refresh: Option<Nanos>,
    pub(crate) flags: PresentFlags,
}

impl SurfaceState {
    /// Presents the latched update at a refresh.
    pub(crate) fn present(&mut self, frame: Frame, book: &mut CallbackBook, out: &mut Vec<Action>) {
        let Some(entry) = self.locked.take() else {
            return;
        };
        let commit = entry.commit;
        let mut shown = std::mem::take(&mut self.discards);
        shown.push(entry);
        let content = shown.iter().rev().find(|e| e.has_content).map(|e| e.commit);
        out.push(Action::Presented {
            surface: self.id,
            commit,
            seq: frame.seq,
            time: frame.time,
            refresh: frame.refresh,
            flags: frame.flags,
            content,
        });
        if let Some(new) = content {
            if let Some(old) = self.held.replace(new) {
                out.push(Action::Released {
                    surface: self.id,
                    commit: old,
                });
            }
            for e in shown.iter().filter(|e| e.has_content && e.commit != new) {
                out.push(Action::Released {
                    surface: self.id,
                    commit: e.commit,
                });
            }
        }
        for e in shown
            .iter()
            .filter(|e| e.wants_callback && !e.callback_armed)
        {
            book.arm(self.id, e.commit, frame.time);
        }
        if self.barrier_clears_at == Some(frame.seq) {
            self.barrier_clears_at = None;
            out.push(Action::BarrierCleared { surface: self.id });
        }
    }
}
