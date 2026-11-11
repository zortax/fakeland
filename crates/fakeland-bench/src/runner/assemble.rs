//! Assembles the run: analysis of the measured iterations, the validity checks and the report.

use fakeland::{OutputState, Session};
use fakeland_types::{Dur, Nanos, Timestamp};

use super::{Bench, BenchRun, IterationRecord};
use crate::input::AnalysisInput;
use crate::report::{BenchConfig, BenchReport, REPORT_FORMAT_VERSION};
use crate::validity::{HostLoad, LatenessStats, SyncStats, ValidityInputs, evaluate, sync_of};

/// The facts of a finished run that the report needs.
pub(crate) struct Finished {
    /// The outputs when the bench started.
    pub outputs: Vec<OutputState>,
    /// Every iteration.
    pub records: Vec<IterationRecord>,
    /// The host load over the run.
    pub host: Option<HostLoad>,
    /// The lateness threshold.
    pub threshold: Nanos,
    /// The start time.
    pub started: Timestamp,
    /// The end time.
    pub finished: Timestamp,
}

/// Analyzes the measured iterations and builds the run with its report.
pub(crate) fn assemble(bench: &Bench, session: &Session, done: Finished) -> BenchRun {
    let Finished {
        outputs,
        records,
        host,
        threshold,
        started,
        finished,
    } = done;
    let measured: Vec<&IterationRecord> = records
        .iter()
        .filter(|r| r.meta.measured.is_some())
        .collect();
    let inputs: Vec<&AnalysisInput> = measured.iter().filter_map(|r| r.input.as_ref()).collect();
    let sections = bench.analyzers.sections(&inputs, bench.seed);
    let mut lateness = LatenessStats::default();
    let mut sync = SyncStats::default();
    for r in &measured {
        lateness.merge(r.lateness);
        if let Some(i) = &r.input {
            sync.merge(sync_of(&i.timeline, i.target_surfaces()));
        }
    }
    let validity = evaluate(
        &bench.validity,
        &ValidityInputs {
            measured: measured.len() as u32,
            failed: measured
                .iter()
                .filter_map(|r| Some((r.meta.index, r.error.clone()?)))
                .collect(),
            lateness,
            threshold,
            host,
            sync,
            samples: sections
                .iter()
                .map(|s| (s.name.clone(), s.samples))
                .collect(),
            dropped_records: inputs.iter().map(|i| i.trace.header.dropped).sum(),
        },
    );
    let report = BenchReport {
        format_version: REPORT_FORMAT_VERSION,
        name: bench.name.clone(),
        preset: bench.preset.clone(),
        params: bench.params.clone(),
        environment: session.environment().clone(),
        config: BenchConfig {
            iterations: bench.iterations,
            warmup: bench.warmup,
            settle: bench.settle,
            quiet: bench.quiet,
            phase: bench.phase,
            seed: bench.seed,
            variants: bench.variants.iter().map(|v| v.label.clone()).collect(),
            target: bench.target.clone(),
            analyzers: bench.analyzers.names(),
            outputs,
            lateness_threshold: Dur(threshold),
        },
        started,
        finished,
        iterations: records.iter().map(IterationRecord::summary).collect(),
        validity,
        sections,
    };
    BenchRun::new(report, records)
}
