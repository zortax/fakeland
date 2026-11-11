//! Processing of deadlines: the advance loop and the refresh of each mode.

use fakeland_types::Timestamp;

use super::Scheduler;
use super::deadline::{Deadline, Kind};
use super::mode_state::ModeState;
use crate::{Action, CallbackPolicy, GridPoint};

impl Scheduler {
    /// Processes every deadline at or before `now` in order and returns the actions.
    pub fn advance(&mut self, now: Timestamp) -> Vec<Action> {
        let mut out = Vec::new();
        self.advance_into(now, &mut out);
        out
    }

    pub(super) fn advance_into(&mut self, now: Timestamp, out: &mut Vec<Action>) {
        while let Some(deadline) = self.next_deadline() {
            if deadline.at > now {
                break;
            }
            self.cursor = deadline.at;
            self.process(deadline, now, out);
        }
    }

    fn process(&mut self, deadline: Deadline, now: Timestamp, out: &mut Vec<Action>) {
        let at = deadline.at;
        match deadline.kind {
            Kind::Apply => self.apply_all(at, out),
            Kind::Dispatch => self.dispatch(at, out),
            Kind::Latch => self.fixed_latch(at, out),
            Kind::Frame => match self.mode {
                ModeState::Fixed { .. } => self.fixed_vblank(now, out),
                ModeState::Vrr { .. } => self.variable_frame(at, now, out),
                ModeState::Unthrottled => self.unthrottled_present(at, now, out),
                ModeState::Manual { .. } => {}
            },
        }
    }

    fn fixed_latch(&mut self, at: Timestamp, out: &mut Vec<Action>) {
        let ModeState::Fixed { grid, .. } = self.mode else {
            return;
        };
        let seq = self.seq + 1;
        if !self.cfg.drops(seq) {
            self.latch_all(seq, at, grid.vblank_time(seq), out);
        }
        if let ModeState::Fixed { latched, .. } = &mut self.mode {
            *latched = true;
        }
    }

    fn fixed_vblank(&mut self, now: Timestamp, out: &mut Vec<Action>) {
        let ModeState::Fixed { grid, .. } = self.mode else {
            return;
        };
        let seq = self.seq + 1;
        let time = grid.vblank_time(seq);
        let presents = self.begin_frame(seq, time, now, out);
        if presents {
            self.present_all(seq, time, Some(grid.period()), true, out);
        }
        self.finish_frame(seq, time);
        if let Some(cfg) = self.pending_cfg.take() {
            self.install(cfg, time);
        }
        if presents {
            self.apply_all(time, out);
            if !matches!(self.cfg.callbacks, CallbackPolicy::BeforeVblank { .. }) {
                self.dispatch(time, out);
            }
        }
    }

    fn variable_frame(&mut self, at: Timestamp, now: Timestamp, out: &mut Vec<Action>) {
        let refresh = self.mode.refresh();
        let seq = self.seq + 1;
        if self.begin_frame(seq, at, now, out) {
            self.latch_all(seq, at, at, out);
            self.present_all(seq, at, refresh, true, out);
            self.finish_frame(seq, at);
            self.apply_all(at, out);
            self.dispatch(at, out);
        } else {
            self.finish_frame(seq, at);
        }
    }

    fn unthrottled_present(&mut self, at: Timestamp, now: Timestamp, out: &mut Vec<Action>) {
        let seq = self.seq + 1;
        self.begin_frame_unthrottled(seq, at, now, out);
        self.latch_all(seq, at, at, out);
        self.present_all(seq, at, None, false, out);
        self.finish_frame(seq, at);
        self.dispatch(at, out);
    }

    /// Runs one manual step at `at`.
    pub(super) fn manual_frame(&mut self, at: Timestamp, out: &mut Vec<Action>) {
        let refresh = self.mode.refresh();
        let seq = self.seq + 1;
        self.cursor = self.cursor.max(at);
        if self.begin_frame(seq, at, at, out) {
            self.latch_all(seq, at, at, out);
            self.present_all(seq, at, refresh, true, out);
            self.finish_frame(seq, at);
            self.apply_all(at, out);
            self.dispatch(at, out);
        } else {
            self.finish_frame(seq, at);
        }
    }

    fn begin_frame_unthrottled(
        &mut self,
        seq: u64,
        ideal: Timestamp,
        woke: Timestamp,
        out: &mut Vec<Action>,
    ) {
        out.push(Action::Vblank {
            seq,
            ideal,
            woke,
            dropped: false,
        });
    }

    /// Advances the sequence number and the mode state past a refresh.
    fn finish_frame(&mut self, seq: u64, time: Timestamp) {
        self.seq = seq;
        self.last_frame = Some(GridPoint { seq, time });
        match &mut self.mode {
            ModeState::Fixed { latched, .. } => *latched = false,
            ModeState::Vrr { last, .. } | ModeState::Manual { last, .. } => *last = time,
            ModeState::Unthrottled => {}
        }
    }
}
