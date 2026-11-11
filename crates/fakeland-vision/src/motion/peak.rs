//! Peak search on a circular correlation surface with parabolic subpixel refinement.

/// The best displacement on a correlation surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Peak {
    /// Horizontal displacement with subpixel refinement.
    pub dx: f64,
    /// Vertical displacement with subpixel refinement.
    pub dy: f64,
    /// Surface value at the integer peak.
    pub value: f64,
    /// The largest value outside the 5 by 5 neighborhood of the peak, within the search window.
    pub second: f64,
    /// Root mean square of the whole surface.
    pub rms: f64,
}

/// Radius of the neighborhood that belongs to the main peak.
const PEAK_RADIUS: i64 = 2;

/// Finds the largest value of a circular `w` by `h` surface for displacements up to `max_dx` and
/// `max_dy`. Index `(x, y)` holds displacement `(x, y)` modulo the size.
pub(crate) fn find_peak(surface: &[f32], w: usize, h: usize, max_dx: usize, max_dy: usize) -> Peak {
    let mx = max_dx.min((w - 1) / 2) as i64;
    let my = max_dy.min((h - 1) / 2) as i64;
    let at = |sx: i64, sy: i64| -> f64 {
        let x = sx.rem_euclid(w as i64) as usize;
        let y = sy.rem_euclid(h as i64) as usize;
        surface[y * w + x] as f64
    };
    let (mut px, mut py, mut best) = (0i64, 0i64, f64::NEG_INFINITY);
    for sy in -my..=my {
        for sx in -mx..=mx {
            let v = at(sx, sy);
            if v > best {
                (px, py, best) = (sx, sy, v);
            }
        }
    }
    let mut second = f64::NEG_INFINITY;
    for sy in -my..=my {
        for sx in -mx..=mx {
            if (sx - px).abs() > PEAK_RADIUS || (sy - py).abs() > PEAK_RADIUS {
                second = second.max(at(sx, sy));
            }
        }
    }
    if second == f64::NEG_INFINITY {
        second = 0.0;
    }
    let fx = if mx > 0 {
        parabolic(best, at(px - 1, py), at(px + 1, py))
    } else {
        0.0
    };
    let fy = if my > 0 {
        parabolic(best, at(px, py - 1), at(px, py + 1))
    } else {
        0.0
    };
    let rms =
        (surface.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / surface.len() as f64).sqrt();
    Peak {
        dx: px as f64 + fx,
        dy: py as f64 + fy,
        value: best,
        second,
        rms,
    }
}

/// Returns the three-point parabola vertex offset, clamped to half a sample.
pub(crate) fn parabolic(c0: f64, cm: f64, cp: f64) -> f64 {
    let denom = cm - 2.0 * c0 + cp;
    if denom >= 0.0 {
        return 0.0;
    }
    (0.5 * (cm - cp) / denom).clamp(-0.5, 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parabola_vertex() {
        // y = -(x - 0.25)^2 sampled at -1, 0, 1.
        let f = |x: f64| -(x - 0.25) * (x - 0.25);
        assert!((parabolic(f(0.0), f(-1.0), f(1.0)) - 0.25).abs() < 1e-12);
        assert_eq!(parabolic(1.0, 2.0, 2.0), 0.0);
    }

    #[test]
    fn finds_wrapped_peak() {
        let (w, h) = (16, 12);
        let mut s = vec![0f32; w * h];
        // Displacement (-3, 2) lives at (13, 2).
        s[2 * w + 13] = 1.0;
        s[2 * w + 12] = 0.2;
        s[9 * w + 5] = 0.5;
        let p = find_peak(&s, w, h, 7, 5);
        assert!((p.dx - (-3.0 - 0.5 * 0.2 / 1.8)).abs() < 1e-6);
        assert_eq!(p.dy, 2.0);
        assert_eq!(p.value, 1.0);
        assert_eq!(p.second, 0.5);
        // A window that excludes the main peak finds the other one.
        let p = find_peak(&s, w, h, 2, 5);
        assert_eq!(p.value, 0.0);
    }
}
