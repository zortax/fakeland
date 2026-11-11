//! The result of a bench run: every iteration with its trace and the report.

use fakeland_trace::Trace;

use super::IterationRecord;
use crate::input::IterationMeta;
use crate::report::BenchReport;

/// A finished bench run.
#[derive(Clone, Debug)]
pub struct BenchRun {
    report: BenchReport,
    iterations: Vec<IterationRecord>,
}

impl BenchRun {
    pub(crate) fn new(report: BenchReport, iterations: Vec<IterationRecord>) -> Self {
        Self { report, iterations }
    }

    /// Returns the report.
    pub fn report(&self) -> BenchReport {
        self.report.clone()
    }

    /// Returns the report by reference.
    pub fn report_ref(&self) -> &BenchReport {
        &self.report
    }

    /// Returns the report and drops the traces.
    pub fn into_report(self) -> BenchReport {
        self.report
    }

    /// Returns every iteration in order, warmups included.
    pub fn iterations(&self) -> &[IterationRecord] {
        &self.iterations
    }

    /// Returns the measured iterations in order.
    pub fn measured(&self) -> impl Iterator<Item = &IterationRecord> + '_ {
        self.iterations.iter().filter(|r| r.meta.measured.is_some())
    }

    /// Returns the trace of every iteration that kept one, with its metadata.
    pub fn traces(&self) -> Vec<(&IterationMeta, &Trace)> {
        self.iterations
            .iter()
            .filter_map(|r| Some((&r.meta, &r.input.as_ref()?.trace)))
            .collect()
    }
}
