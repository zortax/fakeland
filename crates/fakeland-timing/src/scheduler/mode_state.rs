//! The running state of the active refresh mode.

use fakeland_types::{Nanos, Timestamp};

use crate::{Grid, TimingMode};

/// The clock state of the active mode.
#[derive(Clone, Copy, Debug)]
pub(super) enum ModeState {
    /// A fixed grid. `latched` is true once the latch of the next vblank ran.
    Fixed { grid: Grid, latched: bool },
    /// Variable refresh since the last frame.
    Vrr {
        last: Timestamp,
        min_period: Nanos,
        max_period: Nanos,
    },
    /// Presents at readiness.
    Unthrottled,
    /// Vblanks on demand since the last step.
    Manual { last: Timestamp, period: Nanos },
}

impl ModeState {
    /// Builds the state of a mode whose last refresh `seq` happened at `at`.
    pub(super) fn anchored(mode: &TimingMode, at: Timestamp, seq: u64) -> Self {
        match *mode {
            TimingMode::Fixed { refresh } => Self::Fixed {
                grid: Grid::new(at, seq, refresh.period()),
                latched: false,
            },
            TimingMode::Vrr { min, max } => Self::Vrr {
                last: at,
                min_period: max.period(),
                max_period: min.period(),
            },
            TimingMode::Unthrottled => Self::Unthrottled,
            TimingMode::Manual { nominal } => Self::Manual {
                last: at,
                period: nominal.period(),
            },
        }
    }

    /// Returns the grid of the mode. VRR and manual grids predict from the last frame with the
    /// shortest or nominal period.
    pub(super) fn grid(&self, seq: u64) -> Option<Grid> {
        match *self {
            Self::Fixed { grid, .. } => Some(grid),
            Self::Vrr {
                last, min_period, ..
            } => Some(Grid::new(last, seq, min_period)),
            Self::Unthrottled => None,
            Self::Manual { last, period } => Some(Grid::new(last, seq, period)),
        }
    }

    /// Returns the refresh period that presentation feedback reports.
    pub(super) fn refresh(&self) -> Option<Nanos> {
        match *self {
            Self::Fixed { grid, .. } => Some(grid.period()),
            Self::Vrr { min_period, .. } => Some(min_period),
            Self::Unthrottled => None,
            Self::Manual { period, .. } => Some(period),
        }
    }
}
