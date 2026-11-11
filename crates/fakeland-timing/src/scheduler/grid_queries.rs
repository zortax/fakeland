//! Grid queries for scenario anchors: periods, vblank times, sequence numbers, phases and latch
//! deadlines. Fixed outputs answer exactly. VRR and manual outputs predict from the last frame.

use fakeland_types::{Nanos, Timestamp};

use super::Scheduler;
use super::mode_state::ModeState;
use crate::{Grid, GridPoint};

impl Scheduler {
    /// Returns the current grid, or `None` while the output is off or unthrottled.
    pub fn grid(&self) -> Option<Grid> {
        if !self.enabled {
            return None;
        }
        self.mode.grid(self.seq)
    }

    /// Returns the refresh period that presentation feedback reports.
    pub fn refresh(&self) -> Option<Nanos> {
        self.mode.refresh()
    }

    /// Returns the period of the current grid.
    pub fn period(&self) -> Option<Nanos> {
        self.grid().map(|g| g.period())
    }

    /// Returns the ideal time of a vblank on the current grid.
    pub fn vblank_time(&self, seq: u64) -> Option<Timestamp> {
        self.grid().map(|g| g.vblank_time(seq))
    }

    /// Returns the sequence number of the last vblank at or before `t`.
    pub fn seq_at(&self, t: Timestamp) -> Option<u64> {
        self.grid().map(|g| g.seq_at(t))
    }

    /// Returns the first vblank strictly after `t`.
    pub fn next_vblank_after(&self, t: Timestamp) -> Option<GridPoint> {
        self.grid().map(|g| g.next_vblank_after(t))
    }

    /// Returns the span since the last vblank at or before `t`.
    pub fn phase_of(&self, t: Timestamp) -> Option<Nanos> {
        self.grid().map(|g| g.phase_of(t))
    }

    /// Returns the latch deadline of a vblank on a fixed grid, with jitter. The deadline lies
    /// between the previous vblank and the vblank itself.
    pub fn latch_deadline(&self, seq: u64) -> Option<Timestamp> {
        let ModeState::Fixed { grid, .. } = self.mode else {
            return None;
        };
        if !self.enabled {
            return None;
        }
        let vblank = grid.vblank_time(seq);
        let previous = grid.vblank_time(seq.saturating_sub(1));
        let latch = vblank - self.cfg.latch_offset + self.cfg.jitter(seq);
        Some(latch.clamp(previous, vblank))
    }
}
