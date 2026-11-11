//! The latency probe operation.

use fakeland::ProbeReport;
use fakeland_scenario::ProbeSpec;

use super::{Controller, OpError};

impl Controller {
    /// Runs a probe and returns its report.
    pub async fn probe(&self, spec: ProbeSpec) -> Result<ProbeReport, OpError> {
        Ok(self.session().probe(spec).await?)
    }
}
