//! Named scalar metrics. Every analyzer report lists its metrics, and the runner aggregates each
//! metric over the measured iterations with a bootstrap confidence interval.

mod aggregate;
mod metric;

pub use aggregate::{Interval, MetricSummary, aggregate};
pub use metric::{Better, Metric, MetricList, Metrics, Unit};
