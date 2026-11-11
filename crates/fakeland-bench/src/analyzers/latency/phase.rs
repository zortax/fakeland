//! Latency by the vblank phase of the input.

use fakeland_trace::stats::median;
use fakeland_types::Nanos;
use serde::{Deserialize, Serialize};

/// The probe repeats whose input fell in one slice of the refresh period.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseBin {
    /// The start of the slice after the vblank.
    pub from_ms: f64,
    /// The end of the slice.
    pub to_ms: f64,
    /// The repeats with a total in the slice.
    pub count: usize,
    /// The median total.
    pub total_p50_ms: Option<f64>,
}

/// Splits `(phase, total)` pairs into `bins` equal slices of the period.
pub fn by_phase(pairs: &[(Nanos, f64)], period: Nanos, bins: u32) -> Vec<PhaseBin> {
    let bins = bins.max(1);
    let p = period.as_millis_f64();
    if p <= 0.0 {
        return Vec::new();
    }
    let width = p / f64::from(bins);
    let period_ns = i128::from(period.as_nanos());
    let bin_of = |phase: Nanos| {
        let ph = i128::from(phase.as_nanos()).rem_euclid(period_ns);
        ((ph * i128::from(bins)) / period_ns).min(i128::from(bins) - 1) as u32
    };
    (0..bins)
        .map(|i| {
            let from = width * f64::from(i);
            let to = from + width;
            let totals: Vec<f64> = pairs
                .iter()
                .filter(|(phase, _)| bin_of(*phase) == i)
                .map(|(_, t)| *t)
                .collect();
            PhaseBin {
                from_ms: from,
                to_ms: to,
                count: totals.len(),
                total_p50_ms: median(&totals),
            }
        })
        .collect()
}
