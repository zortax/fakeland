//! Iteration metadata and named compositor timestamps that a driver records.

use fakeland_types::{Nanos, Timestamp};
use serde::{Deserialize, Serialize};

/// The position and timing of one iteration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IterationMeta {
    /// The index over every iteration of the bench, warmups included, from 0.
    pub index: u32,
    /// The index over measured iterations of the variant; `None` for a warmup.
    pub measured: Option<u32>,
    /// The variant label.
    pub variant: Option<String>,
    /// The planned vblank phase of the first input.
    pub phase: Option<Nanos>,
    /// The seed of the iteration.
    pub seed: u64,
    /// The time the iteration started.
    pub started: Timestamp,
    /// The time the iteration finished.
    pub finished: Timestamp,
}

impl IterationMeta {
    /// Returns true for a warmup iteration.
    pub fn is_warmup(&self) -> bool {
        self.measured.is_none()
    }

    /// Returns the duration of the iteration.
    pub fn duration(&self) -> Nanos {
        self.finished.since(self.started)
    }
}

/// A named compositor timestamp, such as the time a wait condition held.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// The name.
    pub name: String,
    /// The compositor time.
    pub time: Timestamp,
}

impl Observation {
    /// Builds an observation.
    pub fn new(name: impl Into<String>, time: Timestamp) -> Self {
        Self {
            name: name.into(),
            time,
        }
    }
}
