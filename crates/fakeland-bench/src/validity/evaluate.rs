//! The validity checks of a run from its iterations, host load and sections.

use fakeland_trace::{BufferKind, Event, FrameTimeline, SyncKind, Trace};
use fakeland_types::{Nanos, SurfaceId};

use super::{Check, CheckStatus, HostLoad, Validity, ValidityConfig};

/// Output wake lateness over measured iterations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatenessStats {
    /// The count of output wakes: vblanks and the other deadline wakes.
    pub wakes: u64,
    /// The wakes later than the threshold.
    pub late: u64,
    /// The largest lateness.
    pub max: Nanos,
}

impl LatenessStats {
    /// Adds the counts of another set of wakes.
    pub fn merge(&mut self, other: Self) {
        self.wakes += other.wakes;
        self.late += other.late;
        self.max = self.max.max(other.max);
    }
}

/// Returns the wake lateness of the vblank and deadline wake records of a trace.
pub fn lateness_of(trace: &Trace, threshold: Nanos) -> LatenessStats {
    let mut s = LatenessStats::default();
    for r in &trace.records {
        let (Event::Vblank { ideal, woke, .. }
        | Event::DeadlineWake {
            deadline: ideal,
            woke,
            ..
        }) = r.event
        else {
            continue;
        };
        let late = woke.since(ideal).max(Nanos::ZERO);
        s.wakes += 1;
        s.max = s.max.max(late);
        if late > threshold {
            s.late += 1;
        }
    }
    s
}

/// The sync kinds of dmabuf frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// The count of dmabuf commits.
    pub dmabuf: u64,
    /// The dmabuf commits with explicit sync.
    pub explicit: u64,
}

impl SyncStats {
    /// Adds the counts of another set of frames.
    pub fn merge(&mut self, other: Self) {
        self.dmabuf += other.dmabuf;
        self.explicit += other.explicit;
    }
}

/// Counts the dmabuf commits of some surfaces and those with explicit sync.
pub fn sync_of(
    timeline: &FrameTimeline,
    surfaces: impl IntoIterator<Item = SurfaceId>,
) -> SyncStats {
    let mut s = SyncStats::default();
    for surface in surfaces {
        for f in timeline.frames(surface) {
            if f.buffer
                .as_ref()
                .is_some_and(|b| b.kind == BufferKind::Dmabuf)
            {
                s.dmabuf += 1;
                if f.sync == SyncKind::Explicit {
                    s.explicit += 1;
                }
            }
        }
    }
    s
}

/// What the checks read.
#[derive(Clone, Debug, Default)]
pub struct ValidityInputs {
    /// The count of measured iterations.
    pub measured: u32,
    /// The measured iterations whose driver failed, with the error.
    pub failed: Vec<(u32, String)>,
    /// The wake lateness of measured iterations.
    pub lateness: LatenessStats,
    /// The threshold of the lateness check.
    pub threshold: Nanos,
    /// The host load over the run.
    pub host: Option<HostLoad>,
    /// The sync kinds of the target's dmabuf frames.
    pub sync: SyncStats,
    /// The sample count of each section.
    pub samples: Vec<(String, Option<usize>)>,
    /// The trace records the ring lost.
    pub dropped_records: u64,
}

/// Runs every check.
pub fn evaluate(cfg: &ValidityConfig, input: &ValidityInputs) -> Validity {
    let mut checks = Vec::new();
    let measured = f64::from(input.measured);
    let min_it = f64::from(cfg.min_iterations.max(1));
    checks.push(if input.measured >= cfg.min_iterations.max(1) {
        Check::new(
            "iterations",
            CheckStatus::Pass,
            format!("{} measured iterations", input.measured),
        )
        .with_value(measured, min_it)
    } else {
        Check::new(
            "iterations",
            CheckStatus::Fail,
            format!(
                "{} measured iterations, {} needed",
                input.measured, cfg.min_iterations
            ),
        )
        .with_value(measured, min_it)
    });
    checks.push(if input.failed.is_empty() {
        Check::new(
            "iteration_errors",
            CheckStatus::Pass,
            "every iteration finished",
        )
    } else {
        let (i, e) = &input.failed[0];
        Check::new(
            "iteration_errors",
            CheckStatus::Fail,
            format!(
                "{} iterations failed; iteration {i}: {e}",
                input.failed.len()
            ),
        )
        .with_value(input.failed.len() as f64, 0.0)
    });
    let l = input.lateness;
    let ms = |n: Nanos| n.as_millis_f64();
    let late_status = if l.late > cfg.max_late_wakes {
        if cfg.lateness_fails {
            CheckStatus::Fail
        } else {
            CheckStatus::Warn
        }
    } else {
        CheckStatus::Pass
    };
    checks.push(
        Check::new(
            "wake_lateness",
            late_status,
            format!(
                "{} of {} output wakes later than {:.2} ms; max {:.3} ms",
                l.late,
                l.wakes,
                ms(input.threshold),
                ms(l.max)
            ),
        )
        .with_value(ms(l.max), ms(input.threshold)),
    );
    if let Some(h) = &input.host {
        let status = if h.other_busy > cfg.max_host_busy {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };
        checks.push(
            Check::new(
                "host_load",
                status,
                format!(
                    "other processes used {:.1}% of {} CPUs (all: {:.1}%)",
                    h.other_busy * 100.0,
                    h.cpus,
                    h.busy * 100.0
                ),
            )
            .with_value(h.other_busy, cfg.max_host_busy),
        );
    }
    if input.sync.dmabuf > 0 {
        let fraction = input.sync.explicit as f64 / input.sync.dmabuf as f64;
        let status = if fraction < cfg.min_explicit_sync {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };
        checks.push(
            Check::new(
                "explicit_sync",
                status,
                format!(
                    "{} of {} dmabuf frames use explicit sync; without it GPU time reads as zero",
                    input.sync.explicit, input.sync.dmabuf
                ),
            )
            .with_value(fraction, cfg.min_explicit_sync),
        );
    }
    if cfg.min_samples > 0 {
        for (name, samples) in &input.samples {
            let Some(n) = samples else { continue };
            let status = if *n < cfg.min_samples {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            };
            checks.push(
                Check::new(
                    &format!("samples.{name}"),
                    status,
                    format!("{n} samples, {} wanted", cfg.min_samples),
                )
                .with_value(*n as f64, cfg.min_samples as f64),
            );
        }
    }
    if input.dropped_records > 0 {
        checks.push(
            Check::new(
                "trace_dropped",
                CheckStatus::Warn,
                format!(
                    "the trace ring lost {} records; raise the trace capacity",
                    input.dropped_records
                ),
            )
            .with_value(input.dropped_records as f64, 0.0),
        );
    }
    Validity::from_checks(checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::{Environment, TraceHeader};
    use fakeland_types::{OutputId, Timestamp};

    fn base() -> ValidityInputs {
        ValidityInputs {
            measured: 3,
            threshold: Nanos::from_millis(2),
            ..ValidityInputs::default()
        }
    }

    #[test]
    fn clean_run_is_valid() {
        let v = evaluate(&ValidityConfig::default(), &base());
        assert!(v.valid);
        assert!(v.with_status(CheckStatus::Fail).next().is_none());
    }

    #[test]
    fn late_wakes_and_errors_fail() {
        let mut i = base();
        i.lateness = LatenessStats {
            wakes: 100,
            late: 2,
            max: Nanos::from_millis(5),
        };
        i.failed.push((1, "boom".into()));
        let v = evaluate(&ValidityConfig::default(), &i);
        assert!(!v.valid);
        assert_eq!(v.check("wake_lateness").unwrap().status, CheckStatus::Fail);
        assert_eq!(v.check("wake_lateness").unwrap().value, Some(5.0));
        assert_eq!(
            v.check("iteration_errors").unwrap().status,
            CheckStatus::Fail
        );
        let tolerant = ValidityConfig {
            max_late_wakes: 2,
            ..ValidityConfig::default()
        };
        assert_eq!(
            evaluate(&tolerant, &i)
                .check("wake_lateness")
                .unwrap()
                .status,
            CheckStatus::Pass
        );
    }

    #[test]
    fn warnings_keep_the_run_valid() {
        let mut i = base();
        i.host = Some(HostLoad {
            busy: 0.9,
            other_busy: 0.6,
            cpus: 8,
            loadavg: None,
        });
        i.sync = SyncStats {
            dmabuf: 10,
            explicit: 1,
        };
        i.samples = vec![("latency".into(), Some(3)), ("pacing".into(), None)];
        i.dropped_records = 5;
        let cfg = ValidityConfig {
            min_samples: 10,
            ..ValidityConfig::default()
        };
        let v = evaluate(&cfg, &i);
        assert!(v.valid);
        let warned: Vec<&str> = v
            .with_status(CheckStatus::Warn)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            warned,
            [
                "host_load",
                "explicit_sync",
                "samples.latency",
                "trace_dropped"
            ]
        );
    }

    #[test]
    fn too_few_iterations_fail() {
        let mut i = base();
        i.measured = 0;
        assert!(!evaluate(&ValidityConfig::default(), &i).valid);
    }

    #[test]
    fn lateness_from_vblank_records() {
        let mut t = Trace::new(TraceHeader::new(
            Environment::default(),
            vec![],
            Timestamp(0),
        ));
        for (i, late_us) in [100, 2500, 50].into_iter().enumerate() {
            let ideal = Timestamp(1_000_000 * (i as u64 + 1));
            t.push(
                ideal + Nanos::from_micros(late_us),
                Event::Vblank {
                    output: OutputId(0),
                    seq: i as u64,
                    ideal,
                    woke: ideal + Nanos::from_micros(late_us),
                    dropped: false,
                },
            );
        }
        let s = lateness_of(&t, Nanos::from_millis(2));
        assert_eq!((s.wakes, s.late, s.max), (3, 1, Nanos::from_micros(2500)));
        let deadline = Timestamp(4_000_000);
        t.push(
            deadline + Nanos::from_micros(3100),
            Event::DeadlineWake {
                output: OutputId(0),
                deadline,
                woke: deadline + Nanos::from_micros(3100),
            },
        );
        let s = lateness_of(&t, Nanos::from_millis(2));
        assert_eq!((s.wakes, s.late, s.max), (4, 2, Nanos::from_micros(3100)));
    }
}
