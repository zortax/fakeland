//! Inputs of the scheduler: submitted updates, fence signals, removed surfaces, mode switches,
//! output power and manual steps.

use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::mode_state::ModeState;
use super::{ScheduleError, Scheduler};
use crate::surface::{Entry, SurfaceState};
use crate::{Action, ConfigError, TimingConfig, Update};

impl Scheduler {
    /// Queues an update that a client committed at `now`. Updates of a surface apply in
    /// submission order.
    pub fn submit(&mut self, now: Timestamp, update: Update) {
        self.surfaces
            .entry(update.surface)
            .or_insert_with(|| SurfaceState::new(update.surface))
            .push(Entry::new(&update, now));
    }

    /// Records that the fence of a pending commit signaled at `at`. Returns false when no queued
    /// update has this commit.
    pub fn mark_ready(&mut self, commit: CommitId, at: Timestamp) -> bool {
        self.surfaces
            .values_mut()
            .any(|surface| surface.mark_ready(commit, at))
    }

    /// Removes a surface. Its unfinished updates are discarded, its buffers released and its
    /// callbacks dropped.
    pub fn remove_surface(&mut self, surface: SurfaceId) -> Vec<Action> {
        let mut out = Vec::new();
        self.book.remove_surface(surface);
        if let Some(state) = self.surfaces.remove(&surface) {
            state.remove(&mut out);
        }
        out
    }

    /// Switches the configuration. A running fixed grid switches at its next vblank, so that
    /// vblank keeps its time and the new grid starts there. Every other mode switches at `now`.
    pub fn reconfigure(&mut self, cfg: TimingConfig, now: Timestamp) -> Result<(), ConfigError> {
        cfg.validate()?;
        if self.enabled && matches!(self.mode, ModeState::Fixed { .. }) {
            self.pending_cfg = Some(cfg);
        } else {
            self.pending_cfg = None;
            self.install(cfg, now.max(self.cursor));
        }
        Ok(())
    }

    /// Turns the output on or off. An output that is off has no refreshes and holds every update
    /// and callback. An output that turns on starts its grid at `now`.
    pub fn set_enabled(&mut self, enabled: bool, now: Timestamp) {
        if enabled == self.enabled {
            return;
        }
        self.enabled = enabled;
        if enabled {
            self.cursor = self.cursor.max(now);
            let cfg = self.pending_cfg.take().unwrap_or(self.cfg);
            self.install(cfg, self.cursor);
        }
    }

    /// Runs `n` manual refreshes at `now`. Each refresh latches every update that is ready by
    /// `now`, presents it and fires the due callbacks.
    pub fn step(&mut self, n: u32, now: Timestamp) -> Result<Vec<Action>, ScheduleError> {
        if !matches!(self.mode, ModeState::Manual { .. }) {
            return Err(ScheduleError::NotManual);
        }
        if !self.enabled {
            return Err(ScheduleError::Disabled);
        }
        let mut out = Vec::new();
        self.advance_into(now, &mut out);
        for _ in 0..n {
            self.manual_frame(now, &mut out);
            self.advance_into(now, &mut out);
        }
        Ok(out)
    }

    /// Makes a configuration active with its last refresh at `at`.
    pub(super) fn install(&mut self, cfg: TimingConfig, at: Timestamp) {
        self.cfg = cfg;
        self.mode = ModeState::anchored(&cfg.mode, at, self.seq);
    }
}
