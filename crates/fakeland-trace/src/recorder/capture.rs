//! Capture span ids.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An open capture span of a recorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaptureId(pub u64);

impl fmt::Display for CaptureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capture#{}", self.0)
    }
}
