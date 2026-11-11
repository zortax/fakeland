//! The scheduler state and its read-only queries.

use std::collections::BTreeMap;

use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::mode_state::ModeState;
use crate::callbacks::CallbackBook;
use crate::surface::SurfaceState;
use crate::{ConfigError, GridPoint, TimingConfig};

/// The frame scheduler of one output.
///
/// The caller submits updates, sleeps until [`Scheduler::next_wakeup`] and calls
/// [`Scheduler::advance`] with the current time. The scheduler processes every deadline up to
/// that time in order and returns the actions. Sequence numbers count refreshes from 0 at
/// construction and never restart.
#[derive(Clone, Debug)]
pub struct Scheduler {
    pub(super) cfg: TimingConfig,
    pub(super) pending_cfg: Option<TimingConfig>,
    pub(super) enabled: bool,
    pub(super) mode: ModeState,
    pub(super) seq: u64,
    pub(super) last_frame: Option<GridPoint>,
    pub(super) cursor: Timestamp,
    pub(super) surfaces: BTreeMap<SurfaceId, SurfaceState>,
    pub(super) book: CallbackBook,
}

impl Scheduler {
    /// Builds a scheduler whose grid starts at `start`. The first vblank is sequence 1 at `start`
    /// plus one period.
    pub fn new(cfg: TimingConfig, start: Timestamp) -> Result<Self, ConfigError> {
        cfg.validate()?;
        Ok(Self {
            mode: ModeState::anchored(&cfg.mode, start, 0),
            cfg,
            pending_cfg: None,
            enabled: true,
            seq: 0,
            last_frame: None,
            cursor: start,
            surfaces: BTreeMap::new(),
            book: CallbackBook::default(),
        })
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &TimingConfig {
        &self.cfg
    }

    /// Returns the configuration that takes effect at the next vblank.
    pub fn pending_config(&self) -> Option<&TimingConfig> {
        self.pending_cfg.as_ref()
    }

    /// Returns true while the output is on.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the sequence number of the last refresh.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Returns the last refresh.
    pub fn last_vblank(&self) -> Option<GridPoint> {
        self.last_frame
    }

    /// Returns the time of the last processed deadline.
    pub fn processed_until(&self) -> Timestamp {
        self.cursor
    }

    /// Returns the number of updates of a surface that have not reached the screen.
    pub fn unfinished(&self, surface: SurfaceId) -> usize {
        self.surfaces
            .get(&surface)
            .map_or(0, SurfaceState::unfinished)
    }

    /// Returns the commit whose buffer a surface shows.
    pub fn on_screen(&self, surface: SurfaceId) -> Option<CommitId> {
        self.surfaces.get(&surface).and_then(SurfaceState::held)
    }

    /// Returns the earliest deadline: a latch, a vblank, a callback dispatch, a VRR frame or an
    /// update that applies ahead of the latch. Returns `None` when nothing is due, for example
    /// while the output is off.
    pub fn next_wakeup(&self) -> Option<Timestamp> {
        self.next_deadline().map(|d| d.at)
    }
}
