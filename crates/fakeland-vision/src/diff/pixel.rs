//! The per-pixel change measure: the largest absolute difference of the red, green and blue
//! channels. Alpha does not count.

/// Returns the largest absolute channel difference of two RGBA pixels, alpha excluded.
#[inline(always)]
pub(crate) fn max_rgb_delta(a: &[u8], b: &[u8]) -> u8 {
    a[0].abs_diff(b[0])
        .max(a[1].abs_diff(b[1]))
        .max(a[2].abs_diff(b[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_alpha() {
        assert_eq!(max_rgb_delta(&[10, 20, 30, 0], &[12, 10, 31, 255]), 10);
        assert_eq!(max_rgb_delta(&[1, 1, 1, 1], &[1, 1, 1, 9]), 0);
    }
}
