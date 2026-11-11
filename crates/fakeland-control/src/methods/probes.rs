//! The latency probe method.

use fakeland::ProbeReport;
use fakeland_scenario::ProbeSpec;

use super::method::method;

method!(
    /// Runs a latency probe on the compositor thread and returns its report.
    Probe, "probe", ProbeSpec => ProbeReport
);
