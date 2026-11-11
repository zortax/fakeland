//! The refresh procedures that every mode shares: latch all surfaces, present all surfaces, apply
//! updates without content and dispatch frame callbacks.

use fakeland_types::{Nanos, Timestamp};

use super::Scheduler;
use crate::surface::Frame;
use crate::{Action, PresentFlags, TimingConfig};

impl Scheduler {
    /// Latches every surface for the refresh `seq` at `frame`.
    pub(super) fn latch_all(
        &mut self,
        seq: u64,
        latch: Timestamp,
        frame: Timestamp,
        out: &mut Vec<Action>,
    ) {
        for surface in self.surfaces.values_mut() {
            surface.latch(seq, latch, frame, out);
        }
    }

    /// Presents the latched update of every surface.
    pub(super) fn present_all(
        &mut self,
        seq: u64,
        time: Timestamp,
        refresh: Option<Nanos>,
        vsync: bool,
        out: &mut Vec<Action>,
    ) {
        let frame = Frame {
            seq,
            time,
            refresh,
            flags: feedback_flags(&self.cfg, vsync),
        };
        for surface in self.surfaces.values_mut() {
            surface.present(frame, &mut self.book, out);
        }
    }

    /// Applies front updates without content of every surface.
    pub(super) fn apply_all(&mut self, at: Timestamp, out: &mut Vec<Action>) {
        let Some(limit) = self.target_limit() else {
            return;
        };
        let seq = self.next_unlatched();
        for surface in self.surfaces.values_mut() {
            surface.apply_open(at, limit, seq, &mut self.book, out);
        }
    }

    /// Fires every callback that is eligible at `at`.
    pub(super) fn dispatch(&mut self, at: Timestamp, out: &mut Vec<Action>) {
        let callbacks = self.book.take_due(at);
        if !callbacks.is_empty() {
            out.push(Action::FrameCallbacks {
                time: at,
                callbacks,
            });
        }
    }

    /// Records a refresh and returns true when it presents, false when it drops.
    pub(super) fn begin_frame(
        &mut self,
        seq: u64,
        ideal: Timestamp,
        woke: Timestamp,
        out: &mut Vec<Action>,
    ) -> bool {
        let dropped = self.cfg.drops(seq);
        out.push(Action::Vblank {
            seq,
            ideal,
            woke,
            dropped,
        });
        !dropped
    }
}

fn feedback_flags(cfg: &TimingConfig, vsync: bool) -> PresentFlags {
    let mut flags = PresentFlags::HW_CLOCK | PresentFlags::HW_COMPLETION;
    if vsync {
        flags |= PresentFlags::VSYNC;
    }
    if cfg.zero_copy {
        flags |= PresentFlags::ZERO_COPY;
    }
    flags
}
