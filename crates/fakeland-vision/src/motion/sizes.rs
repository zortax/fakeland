//! FFT-friendly sizes and the Hann window.

/// Returns the largest value up to `n` whose prime factors are 2, 3 and 5. Zero stays zero.
pub(crate) fn smooth_floor(n: usize) -> usize {
    (1..=n).rev().find(|&m| is_smooth(m)).unwrap_or(0)
}

fn is_smooth(mut m: usize) -> bool {
    for p in [2, 3, 5] {
        while m.is_multiple_of(p) {
            m /= p;
        }
    }
    m == 1
}

/// Writes a periodic Hann window of length `n` into `out`.
pub(crate) fn hann_into(n: usize, out: &mut Vec<f32>) {
    out.clear();
    let step = std::f32::consts::TAU / n as f32;
    out.extend((0..n).map(|i| 0.5 - 0.5 * (step * i as f32).cos()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_sizes() {
        assert_eq!(smooth_floor(256), 256);
        assert_eq!(smooth_floor(241), 240);
        assert_eq!(smooth_floor(135), 135);
        assert_eq!(smooth_floor(127), 125);
        assert_eq!(smooth_floor(7), 6);
        assert_eq!(smooth_floor(1), 1);
        assert_eq!(smooth_floor(0), 0);
    }

    #[test]
    fn hann_shape() {
        let mut w = Vec::new();
        hann_into(8, &mut w);
        assert_eq!(w.len(), 8);
        assert!(w[0].abs() < 1e-6);
        assert!((w[4] - 1.0).abs() < 1e-6);
        assert!((w[2] - w[6]).abs() < 1e-6);
    }
}
