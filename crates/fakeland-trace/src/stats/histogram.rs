//! Histograms with fixed linear or logarithmic bins.

use serde::{Deserialize, Serialize};

/// One histogram bin. A value `v` falls in the bin when `lo <= v < hi`; the last bin also holds
/// `v == hi`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bin {
    /// The lower edge.
    pub lo: f64,
    /// The upper edge.
    pub hi: f64,
    /// The number of values in the bin.
    pub count: u64,
}

/// An error of histogram construction.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum HistogramError {
    /// The bin count is zero.
    #[error("a histogram needs at least one bin")]
    NoBins,
    /// The edges are not finite and strictly increasing.
    #[error("histogram edges must be finite and strictly increasing")]
    BadEdges,
    /// A logarithmic histogram has a lower edge at or below zero.
    #[error("logarithmic bins need a positive lower edge")]
    NonPositiveLog,
}

/// Counts of values in fixed bins, with counts below and above the range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    /// The bins in ascending order.
    pub bins: Vec<Bin>,
    /// The number of values below the first edge.
    pub underflow: u64,
    /// The number of values above the last edge.
    pub overflow: u64,
    /// The number of finite values added.
    pub total: u64,
}

impl Histogram {
    /// Builds an empty histogram from bin edges.
    pub fn with_edges(edges: &[f64]) -> Result<Self, HistogramError> {
        if edges.len() < 2 {
            return Err(HistogramError::NoBins);
        }
        if edges.iter().any(|e| !e.is_finite()) || edges.windows(2).any(|w| w[1] <= w[0]) {
            return Err(HistogramError::BadEdges);
        }
        Ok(Self {
            bins: edges
                .windows(2)
                .map(|w| Bin {
                    lo: w[0],
                    hi: w[1],
                    count: 0,
                })
                .collect(),
            underflow: 0,
            overflow: 0,
            total: 0,
        })
    }

    /// Builds an empty histogram with `bins` equal-width bins over `lo..hi`.
    pub fn linear(lo: f64, hi: f64, bins: usize) -> Result<Self, HistogramError> {
        if bins == 0 {
            return Err(HistogramError::NoBins);
        }
        let step = (hi - lo) / bins as f64;
        let edges: Vec<f64> = (0..=bins)
            .map(|i| if i == bins { hi } else { lo + step * i as f64 })
            .collect();
        Self::with_edges(&edges)
    }

    /// Builds an empty histogram with `bins` bins of equal width in `ln` over `lo..hi`.
    pub fn log(lo: f64, hi: f64, bins: usize) -> Result<Self, HistogramError> {
        if bins == 0 {
            return Err(HistogramError::NoBins);
        }
        if lo <= 0.0 {
            return Err(HistogramError::NonPositiveLog);
        }
        let (a, b) = (lo.ln(), hi.ln());
        let step = (b - a) / bins as f64;
        let edges: Vec<f64> = (0..=bins)
            .map(|i| match i {
                0 => lo,
                i if i == bins => hi,
                i => (a + step * i as f64).exp(),
            })
            .collect();
        Self::with_edges(&edges)
    }

    /// Adds one value. Non-finite values are ignored.
    pub fn add(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        self.total += 1;
        let (Some(first), Some(last)) = (self.bins.first(), self.bins.last()) else {
            return;
        };
        if v < first.lo {
            self.underflow += 1;
        } else if v > last.hi {
            self.overflow += 1;
        } else {
            let i = self.bins.partition_point(|b| b.hi <= v);
            let i = i.min(self.bins.len() - 1);
            self.bins[i].count += 1;
        }
    }

    /// Adds every value and returns the histogram.
    pub fn fill(mut self, values: &[f64]) -> Self {
        for v in values {
            self.add(*v);
        }
        self
    }

    /// Returns the bin with the highest count, the first one on a tie.
    pub fn mode(&self) -> Option<&Bin> {
        self.bins
            .iter()
            .filter(|b| b.count > 0)
            .reduce(|best, b| if b.count > best.count { b } else { best })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_bins_and_edges() {
        let h = Histogram::linear(0.0, 10.0, 5).unwrap().fill(&[
            -1.0,
            0.0,
            1.9,
            2.0,
            9.99,
            10.0,
            11.0,
            f64::NAN,
        ]);
        let counts: Vec<u64> = h.bins.iter().map(|b| b.count).collect();
        assert_eq!(counts, [2, 1, 0, 0, 2]);
        assert_eq!(h.underflow, 1);
        assert_eq!(h.overflow, 1);
        assert_eq!(h.total, 7);
        assert_eq!(h.mode().map(|b| b.lo), Some(0.0));
    }

    #[test]
    fn log_bins() {
        let h = Histogram::log(1.0, 1000.0, 3)
            .unwrap()
            .fill(&[1.0, 5.0, 50.0, 500.0, 999.0]);
        assert!((h.bins[0].hi - 10.0).abs() < 1e-9);
        assert!((h.bins[1].hi - 100.0).abs() < 1e-9);
        let counts: Vec<u64> = h.bins.iter().map(|b| b.count).collect();
        assert_eq!(counts, [2, 1, 2]);
    }

    #[test]
    fn invalid_configurations() {
        assert_eq!(Histogram::linear(0.0, 1.0, 0), Err(HistogramError::NoBins));
        assert_eq!(
            Histogram::linear(1.0, 1.0, 2),
            Err(HistogramError::BadEdges)
        );
        assert_eq!(
            Histogram::log(0.0, 1.0, 2),
            Err(HistogramError::NonPositiveLog)
        );
        assert_eq!(Histogram::with_edges(&[1.0]), Err(HistogramError::NoBins));
    }
}
