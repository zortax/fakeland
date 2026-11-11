//! Statistics over samples: summaries with percentiles, histograms, bootstrap confidence intervals,
//! frame-rate lows and MAD outlier filtering.

mod bootstrap;
mod histogram;
mod lows;
mod outliers;
mod percentile;
mod summary;

pub use bootstrap::{ConfidenceInterval, bootstrap_ci};
pub use histogram::{Bin, Histogram, HistogramError};
pub use lows::FpsLows;
pub use outliers::{MadFilter, mad_filter, median};
pub use percentile::{percentile, percentile_sorted};
pub use summary::Summary;
