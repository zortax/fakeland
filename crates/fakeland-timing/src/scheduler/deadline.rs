//! The next deadline of the scheduler. Deadlines never lie before the last processed one, and
//! ties resolve in the order apply, dispatch, latch, frame.

use fakeland_types::Timestamp;

use super::Scheduler;
use super::mode_state::ModeState;
use crate::CallbackPolicy;
use crate::surface::TargetLimit;

const DISPATCH_SEARCH: u64 = 100_000;

/// What happens at a deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    /// Updates without content apply ahead of the latch.
    Apply,
    /// Frame callbacks fire.
    Dispatch,
    /// The fixed grid latches for the next vblank.
    Latch,
    /// A refresh: a fixed vblank, a VRR frame or an unthrottled present.
    Frame,
}

/// A deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Deadline {
    pub(super) at: Timestamp,
    pub(super) kind: Kind,
}

impl Scheduler {
    pub(super) fn next_deadline(&self) -> Option<Deadline> {
        if !self.enabled {
            return None;
        }
        let mut best: Option<Deadline> = None;
        let mut consider = |at: Option<Timestamp>, kind: Kind| {
            let Some(at) = at else { return };
            let d = Deadline {
                at: at.max(self.cursor),
                kind,
            };
            if best.is_none_or(|b| (d.at, d.kind) < (b.at, b.kind)) {
                best = Some(d);
            }
        };
        if let Some(limit) = self.target_limit() {
            let apply = self
                .surfaces
                .values()
                .filter_map(|s| s.apply_time(limit))
                .min();
            consider(apply, Kind::Apply);
        }
        consider(self.dispatch_time(), Kind::Dispatch);
        match self.mode {
            ModeState::Fixed { grid, latched } => {
                let next = self.seq + 1;
                if !latched {
                    consider(self.latch_deadline(next), Kind::Latch);
                }
                consider(Some(grid.vblank_time(next)), Kind::Frame);
            }
            ModeState::Vrr {
                last,
                min_period,
                max_period,
            } => {
                let content = self
                    .surfaces
                    .values()
                    .filter_map(|s| s.content_need())
                    .min();
                let repeat = last + max_period;
                let frame = content.map_or(repeat, |c| c.max(last + min_period).min(repeat));
                consider(Some(frame), Kind::Frame);
            }
            ModeState::Unthrottled => {
                let present = self.surfaces.values().filter_map(|s| s.any_need()).min();
                consider(present, Kind::Frame);
            }
            ModeState::Manual { .. } => {}
        }
        best
    }

    /// Returns the commit-timing limit for updates that apply ahead of the latch, or `None` when
    /// the mode presents every update at readiness.
    pub(super) fn target_limit(&self) -> Option<TargetLimit> {
        match self.mode {
            ModeState::Fixed { grid, latched } => {
                let next = self.seq + if latched { 2 } else { 1 };
                Some(TargetLimit::Frame(grid.vblank_time(next)))
            }
            ModeState::Vrr { .. } | ModeState::Manual { .. } => Some(TargetLimit::AtApply),
            ModeState::Unthrottled => None,
        }
    }

    /// Returns the sequence number of the next refresh that has not latched.
    pub(super) fn next_unlatched(&self) -> u64 {
        match self.mode {
            ModeState::Fixed { latched: true, .. } => self.seq + 2,
            _ => self.seq + 1,
        }
    }

    fn dispatch_time(&self) -> Option<Timestamp> {
        let eligible = self.book.earliest()?;
        if matches!(self.cfg.callbacks, CallbackPolicy::OnPresent)
            || matches!(self.mode, ModeState::Unthrottled)
        {
            return Some(eligible);
        }
        let (ModeState::Fixed { grid, .. }, CallbackPolicy::BeforeVblank { lead }) =
            (self.mode, self.cfg.callbacks)
        else {
            return None;
        };
        let lead = lead.min(grid.period());
        let first = self.seq + 1;
        (first..first + DISPATCH_SEARCH)
            .map(|k| (k, grid.vblank_time(k) - lead))
            .find(|&(k, at)| at >= eligible && !self.cfg.drops(k))
            .map(|(_, at)| at)
    }
}
