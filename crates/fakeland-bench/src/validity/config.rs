//! The limits of the validity checks.

use fakeland_types::Dur;
use serde::{Deserialize, Serialize};

/// The limits of the validity checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ValidityConfig {
    /// The output wake lateness that makes a run invalid. `None` takes the session threshold.
    pub lateness_threshold: Option<Dur>,
    /// The count of late wakes a run tolerates.
    pub max_late_wakes: u64,
    /// More late wakes fail the run. Benches whose own readbacks delay wakes, and which measure
    /// no display timing, turn this off and get a warning.
    pub lateness_fails: bool,
    /// The CPU time of other processes, as a fraction of every CPU, above which the host counts
    /// as loaded.
    pub max_host_busy: f64,
    /// The fraction of dmabuf frames with explicit sync below which GPU times count as missing.
    pub min_explicit_sync: f64,
    /// The measured iterations a run needs.
    pub min_iterations: u32,
    /// The samples each section with samples needs; 0 skips the check. A section whose analyzer
    /// found no data source reports no sample count and skips the check.
    pub min_samples: usize,
}

impl Default for ValidityConfig {
    fn default() -> Self {
        Self {
            lateness_threshold: None,
            max_late_wakes: 0,
            lateness_fails: true,
            max_host_busy: 0.25,
            min_explicit_sync: 0.5,
            min_iterations: 1,
            min_samples: 10,
        }
    }
}
