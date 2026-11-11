//! The bench report document and its JSON form.

use std::io::{Read, Write};

use fakeland_trace::Environment;
use fakeland_types::{Nanos, Timestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BenchConfig, Section};
use crate::metrics::MetricSummary;
use crate::validity::Validity;

/// The format version of bench reports. A breaking change of the JSON form bumps it.
pub const REPORT_FORMAT_VERSION: u32 = 1;

/// An error while reading or writing a report.
#[derive(Debug, Error)]
pub enum ReportError {
    /// Reading or writing failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The JSON is malformed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// The report has another format version.
    #[error("report format version {found}, expected {expected}")]
    Version {
        /// The version in the report.
        found: u32,
        /// The version this build reads.
        expected: u32,
    },
}

/// The outcome of one iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IterationSummary {
    /// The index over the whole bench, from 0.
    pub index: u32,
    /// The index over measured iterations of the variant; `None` for a warmup.
    pub measured: Option<u32>,
    /// The variant label.
    #[serde(default)]
    pub variant: Option<String>,
    /// The planned vblank phase of the first input.
    #[serde(default)]
    pub phase: Option<Nanos>,
    /// The start time.
    pub started: Timestamp,
    /// The end time.
    pub finished: Timestamp,
    /// The driver finished without an error.
    pub ok: bool,
    /// The driver error.
    #[serde(default)]
    pub error: Option<String>,
    /// The output wakes of the iteration later than the threshold.
    pub late_wakes: u64,
    /// The largest output wake lateness.
    pub max_lateness: Nanos,
    /// The count of trace records.
    pub records: usize,
    /// The count of trace records the ring lost.
    pub dropped_records: u64,
}

/// A bench report. Every time is a compositor timestamp on `CLOCK_MONOTONIC`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchReport {
    /// The report format version, [`REPORT_FORMAT_VERSION`].
    pub format_version: u32,
    /// The bench name.
    pub name: String,
    /// The preset that built the bench.
    #[serde(default)]
    pub preset: Option<String>,
    /// The preset parameters.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// The host environment.
    pub environment: Environment,
    /// The bench settings.
    pub config: BenchConfig,
    /// The time the bench started.
    pub started: Timestamp,
    /// The time the bench finished.
    pub finished: Timestamp,
    /// Every iteration, warmups included.
    pub iterations: Vec<IterationSummary>,
    /// The validity checks.
    pub validity: Validity,
    /// One section per analyzer.
    pub sections: Vec<Section>,
}

impl BenchReport {
    /// Returns true when no validity check failed.
    pub fn is_valid(&self) -> bool {
        self.validity.valid
    }

    /// Returns the section of an analyzer.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Returns a metric summary by section, name and variant.
    pub fn metric(
        &self,
        section: &str,
        name: &str,
        variant: Option<&str>,
    ) -> Option<&MetricSummary> {
        self.section(section)?.metric(name, variant)
    }

    /// Returns the duration of the bench.
    pub fn duration(&self) -> Nanos {
        self.finished.since(self.started)
    }

    /// Returns the report as pretty JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Writes the report as pretty JSON.
    pub fn write_json(&self, mut out: impl Write) -> Result<(), ReportError> {
        serde_json::to_writer_pretty(&mut out, self)?;
        out.write_all(b"\n")?;
        Ok(())
    }

    /// Parses a report and checks its format version.
    pub fn from_json(json: &str) -> Result<Self, ReportError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        Self::from_value(value)
    }

    /// Reads a report and checks its format version.
    pub fn read_json(input: impl Read) -> Result<Self, ReportError> {
        let value: serde_json::Value = serde_json::from_reader(input)?;
        Self::from_value(value)
    }

    fn from_value(value: serde_json::Value) -> Result<Self, ReportError> {
        let found = value
            .get("format_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u32;
        if found != REPORT_FORMAT_VERSION {
            return Err(ReportError::Version {
                found,
                expected: REPORT_FORMAT_VERSION,
            });
        }
        Ok(serde_json::from_value(value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{Better, MetricSummary, Unit};
    use crate::report::Section;
    use crate::validity::{Check, CheckStatus};

    fn report() -> BenchReport {
        BenchReport {
            format_version: REPORT_FORMAT_VERSION,
            name: "demo".into(),
            preset: Some("pacing".into()),
            params: Some(serde_json::json!({"duration": "1s"})),
            environment: Environment {
                gpu: "gpu".into(),
                ..Environment::default()
            },
            config: BenchConfig {
                iterations: 3,
                variants: vec!["60Hz".into()],
                ..BenchConfig::default()
            },
            started: Timestamp(0),
            finished: Timestamp(2_000_000_000),
            iterations: Vec::new(),
            validity: Validity::from_checks(vec![Check::new(
                "wake_lateness",
                CheckStatus::Pass,
                "fine",
            )]),
            sections: vec![Section {
                name: "frame_pacing".into(),
                metrics: vec![
                    MetricSummary::of(
                        "fps",
                        Some("60Hz".into()),
                        Unit::Fps,
                        Better::Higher,
                        vec![59.9, 60.0, 60.1],
                        3,
                    )
                    .unwrap(),
                ],
                samples: Some(180),
                pooled: Vec::new(),
                iterations: Vec::new(),
                error: None,
            }],
        }
    }

    #[test]
    fn json_round_trip_and_version_check() {
        let r = report();
        let back = BenchReport::from_json(&r.to_json_pretty()).unwrap();
        assert_eq!(back, r);
        let mut buf = Vec::new();
        r.write_json(&mut buf).unwrap();
        assert_eq!(BenchReport::read_json(buf.as_slice()).unwrap(), r);
        let mut v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        v["format_version"] = 99.into();
        assert!(matches!(
            BenchReport::from_json(&v.to_string()),
            Err(ReportError::Version { found: 99, .. })
        ));
        assert_eq!(r.metric("frame_pacing", "fps", Some("60Hz")).unwrap().n, 3);
        assert!(r.is_valid());
        assert_eq!(r.duration(), Nanos::from_secs(2));
    }

    #[test]
    fn text_has_the_header_checks_and_tables() {
        let text = report().to_text();
        assert!(text.starts_with("bench demo (preset pacing)\n"), "{text}");
        assert!(text.contains("host: gpu gpu"));
        assert!(text.contains("validity: valid"));
        assert!(text.contains("[pass] wake_lateness: fine"));
        assert!(text.contains("== frame_pacing, 180 samples =="));
        assert!(text.contains("variant"));
        assert!(text.contains("60Hz"));
        assert!(text.contains("59.90 .. 60.10") || text.contains(" .. "));
    }
}
