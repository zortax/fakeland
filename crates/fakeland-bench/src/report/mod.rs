//! Bench reports: the environment, the bench config, the validity checks and one section per
//! analyzer with metrics aggregated over iterations. Reports serialize as JSON with a format
//! version and render as text tables.

mod bench_report;
mod config;
mod section;
pub(crate) mod table;
mod text;

pub use bench_report::{BenchReport, IterationSummary, REPORT_FORMAT_VERSION, ReportError};
pub use config::BenchConfig;
pub use section::{IterationReport, PooledReport, Section};
pub use table::Table;
