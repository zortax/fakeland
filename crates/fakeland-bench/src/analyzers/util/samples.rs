//! Summaries of millisecond samples and the metric values they give.

use fakeland_trace::Summary;
use fakeland_types::Nanos;

/// Returns spans as milliseconds.
pub fn ms_values(spans: impl IntoIterator<Item = Nanos>) -> Vec<f64> {
    spans.into_iter().map(Nanos::as_millis_f64).collect()
}

/// Returns the summary of spans in milliseconds.
pub fn summary_ms(spans: impl IntoIterator<Item = Nanos>) -> Option<Summary> {
    Summary::of_millis(spans)
}

/// Shortcuts to the fields of an optional summary.
pub trait SampleStats {
    /// The median.
    fn p50(&self) -> Option<f64>;
    /// The 95th percentile.
    fn p95(&self) -> Option<f64>;
    /// The 99th percentile.
    fn p99(&self) -> Option<f64>;
    /// The mean.
    fn mean(&self) -> Option<f64>;
    /// The maximum.
    fn max(&self) -> Option<f64>;
    /// The sample count; 0 without a summary.
    fn n(&self) -> usize;
}

impl SampleStats for Option<Summary> {
    fn p50(&self) -> Option<f64> {
        self.map(|s| s.p50)
    }

    fn p95(&self) -> Option<f64> {
        self.map(|s| s.p95)
    }

    fn p99(&self) -> Option<f64> {
        self.map(|s| s.p99)
    }

    fn mean(&self) -> Option<f64> {
        self.map(|s| s.mean)
    }

    fn max(&self) -> Option<f64> {
        self.map(|s| s.max)
    }

    fn n(&self) -> usize {
        self.map_or(0, |s| s.count)
    }
}
