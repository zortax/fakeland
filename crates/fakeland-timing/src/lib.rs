//! Display clock models. Each model turns a refresh configuration into a vblank schedule, latch
//! deadlines and frame-callback times, and resolves fifo and commit-timing barriers. The models are
//! pure and run against any clock.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod action;
mod callbacks;
mod config;
mod flags;
mod grid;
mod scheduler;
mod surface;
mod update;

pub use action::{Action, FrameCallback};
pub use config::{CallbackPolicy, ConfigError, Perturbation, TimingConfig, TimingMode};
pub use flags::PresentFlags;
pub use grid::{Grid, GridPoint};
pub use scheduler::{ScheduleError, Scheduler};
pub use update::{FifoFlags, Readiness, Update};
