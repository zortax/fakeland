//! Mapping of a region of interest onto the buffer of one surface. A surface has a global origin,
//! a logical size and a buffer size in pixels; viewports and buffer scales only change the ratio
//! between the two sizes.

use fakeland_types::{Logical, Physical, Rect};

/// The placement of one surface buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferPlacement {
    /// The global logical origin of the surface.
    pub origin: (i32, i32),
    /// The logical size of the surface.
    pub logical: (f64, f64),
    /// The buffer size in pixels.
    pub buffer: (i32, i32),
}

impl BufferPlacement {
    /// Returns the buffer pixels per logical unit, horizontally.
    pub fn scale(&self) -> f64 {
        if self.logical.0 > 0.0 {
            self.buffer.0 as f64 / self.logical.0
        } else {
            1.0
        }
    }

    /// Returns the part of a global logical region that the buffer covers, in buffer pixels.
    pub fn region(&self, roi: Rect<Logical>) -> Option<Rect<Physical>> {
        let (lw, lh) = self.logical;
        let (bw, bh) = self.buffer;
        if lw <= 0.0 || lh <= 0.0 || bw <= 0 || bh <= 0 {
            return None;
        }
        let x0 = ((roi.x - self.origin.0) as f64).max(0.0);
        let y0 = ((roi.y - self.origin.1) as f64).max(0.0);
        let x1 = ((roi.x + roi.w - self.origin.0) as f64).min(lw);
        let y1 = ((roi.y + roi.h - self.origin.1) as f64).min(lh);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (sx, sy) = (bw as f64 / lw, bh as f64 / lh);
        let px0 = ((x0 * sx).floor() as i32).clamp(0, bw);
        let py0 = ((y0 * sy).floor() as i32).clamp(0, bh);
        let px1 = ((x1 * sx).ceil() as i32).clamp(0, bw);
        let py1 = ((y1 * sy).ceil() as i32).clamp(0, bh);
        (px1 > px0 && py1 > py0).then(|| Rect::new(px0, py0, px1 - px0, py1 - py0))
    }

    /// Returns true when buffer damage touches the region. Damage may reach past the buffer.
    pub fn damage_hits(&self, roi: Rect<Logical>, damage: &[Rect<Physical>]) -> bool {
        let Some(region) = self.region(roi) else {
            return false;
        };
        damage.iter().any(|d| {
            let x0 = d.x.max(0);
            let y0 = d.y.max(0);
            let x1 = d.x.saturating_add(d.w).min(self.buffer.0);
            let y1 = d.y.saturating_add(d.h).min(self.buffer.1);
            x1 > x0 && y1 > y0 && Rect::new(x0, y0, x1 - x0, y1 - y0).intersects(region)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement() -> BufferPlacement {
        BufferPlacement {
            origin: (100, 50),
            logical: (400.0, 300.0),
            buffer: (600, 450),
        }
    }

    #[test]
    fn maps_regions_through_the_scale() {
        let p = placement();
        assert_eq!(p.scale(), 1.5);
        assert_eq!(
            p.region(Rect::new(200, 100, 100, 50)),
            Some(Rect::new(150, 75, 150, 75))
        );
        assert_eq!(
            p.region(Rect::new(0, 0, 150, 70)),
            Some(Rect::new(0, 0, 75, 30))
        );
        assert_eq!(p.region(Rect::new(600, 0, 10, 10)), None);
        assert_eq!(
            p.region(Rect::new(0, 0, 10_000, 10_000)),
            Some(Rect::new(0, 0, 600, 450))
        );
    }

    #[test]
    fn damage_reaching_past_the_buffer_counts() {
        let p = placement();
        let roi = Rect::new(200, 100, 100, 50);
        assert!(p.damage_hits(roi, &[Rect::new(0, 0, i32::MAX, i32::MAX)]));
        assert!(p.damage_hits(roi, &[Rect::new(140, 70, 20, 20)]));
        assert!(!p.damage_hits(roi, &[Rect::new(0, 0, 100, 60)]));
        assert!(!p.damage_hits(roi, &[]));
        assert!(!p.damage_hits(Rect::new(0, 0, 10, 10), &[Rect::new(0, 0, 600, 450)]));
    }
}
