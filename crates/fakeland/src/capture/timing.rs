//! Capture timing. A capture runs on the compositor thread in short steps: the render, which
//! uploads new shm buffers and starts the readback, the map of the pack buffer after the GPU is
//! done, and chunks of row copies. The cost of each step grows with its pixel count, so the
//! session keeps a cost per pixel for each kind of work from earlier captures. The estimates size
//! the copy chunks and place each step clear of output wakes.

use fakeland_types::Nanos;

use super::service::Plan;

/// The longest expected run of one copy chunk.
pub const CHUNK_BUDGET: Nanos = Nanos::from_micros(750);

/// The expected cost of one kind of capture step.
#[derive(Debug, Clone, Copy)]
struct Estimate {
    fixed: Nanos,
    ns_per_pixel: f64,
}

impl Estimate {
    fn expected(&self, pixels: u64) -> Nanos {
        self.fixed + Nanos((self.ns_per_pixel * pixels as f64) as i64)
    }

    /// A higher cost replaces the estimate at once; a lower cost lowers it slowly.
    fn observe(&mut self, pixels: u64, cost: Nanos) {
        if pixels == 0 {
            return;
        }
        let v = (cost - self.fixed).max(Nanos::ZERO).as_nanos() as f64 / pixels as f64;
        self.ns_per_pixel = if v > self.ns_per_pixel {
            v
        } else {
            0.8 * self.ns_per_pixel + 0.2 * v
        };
    }
}

/// The expected capture cost from earlier captures.
#[derive(Debug, Clone, Copy)]
pub struct CaptureCost {
    render: Estimate,
    upload: Estimate,
    map: Estimate,
    copy: Estimate,
}

impl Default for CaptureCost {
    /// The first estimates are a little above the costs of a warmed-up renderer.
    fn default() -> Self {
        Self {
            render: Estimate {
                fixed: Nanos::from_micros(300),
                ns_per_pixel: 0.2,
            },
            upload: Estimate {
                fixed: Nanos::ZERO,
                ns_per_pixel: 1.0,
            },
            map: Estimate {
                fixed: Nanos::from_micros(50),
                ns_per_pixel: 0.5,
            },
            copy: Estimate {
                fixed: Nanos::from_micros(50),
                ns_per_pixel: 2.0,
            },
        }
    }
}

impl CaptureCost {
    /// Returns the expected cost of the render step of `pixels` pixels.
    pub fn render(&self, pixels: u64) -> Nanos {
        self.render.expected(pixels)
    }

    /// Adds the cost of a render step.
    pub fn observe_render(&mut self, pixels: u64, cost: Nanos) {
        self.render.observe(pixels, cost);
    }

    /// Returns the expected cost of uploading `pixels` shm pixels to textures.
    pub fn upload(&self, pixels: u64) -> Nanos {
        if pixels == 0 {
            return Nanos::ZERO;
        }
        self.upload.expected(pixels)
    }

    /// Adds the cost of an upload.
    pub fn observe_upload(&mut self, pixels: u64, cost: Nanos) {
        self.upload.observe(pixels, cost);
    }

    /// Returns the expected cost of mapping a readback of `pixels` pixels.
    pub fn map(&self, pixels: u64) -> Nanos {
        self.map.expected(pixels)
    }

    /// Adds the cost of a map step.
    pub fn observe_map(&mut self, pixels: u64, cost: Nanos) {
        self.map.observe(pixels, cost);
    }

    /// Returns the expected cost of copying `pixels` pixels.
    pub fn copy(&self, pixels: u64) -> Nanos {
        self.copy.expected(pixels)
    }

    /// Adds the cost of a copy chunk.
    pub fn observe_copy(&mut self, pixels: u64, cost: Nanos) {
        self.copy.observe(pixels, cost);
    }

    /// Returns the number of rows of `width` pixels that one copy chunk takes, at least one.
    pub fn chunk_rows(&self, width: u32) -> u32 {
        let budget = (CHUNK_BUDGET - self.copy.fixed).max(Nanos::ZERO).as_nanos() as f64;
        let per_row = self.copy.ns_per_pixel * f64::from(width.max(1));
        if per_row <= 0.0 {
            return u32::MAX;
        }
        ((budget / per_row) as u32).max(1)
    }
}

/// Returns the pixel count of a plan.
pub(super) fn plan_pixels(plan: &Plan) -> u64 {
    let size = super::render::plan_size(plan);
    size.w as u64 * size.h as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_follows_steps() {
        let mut c = CaptureCost::default();
        let px = 1_000_000;
        let fixed = c.render.fixed;
        assert_eq!(c.render(px), fixed + Nanos::from_micros(200));
        c.observe_render(px, fixed + Nanos::from_micros(100));
        assert_eq!(c.render(px), fixed + Nanos::from_micros(180));
        c.observe_render(px, fixed + Nanos::from_millis(3));
        assert_eq!(c.render(px), fixed + Nanos::from_millis(3));
        c.observe_render(0, Nanos::from_secs(1));
        assert_eq!(c.render(px), fixed + Nanos::from_millis(3));
    }

    #[test]
    fn a_chunk_fits_the_budget() {
        let mut c = CaptureCost::default();
        let rows = c.chunk_rows(1920);
        assert!(c.copy(u64::from(rows) * 1920) <= CHUNK_BUDGET);
        assert!(c.copy(u64::from(rows + 1) * 1920) > CHUNK_BUDGET);
        c.observe_copy(1000, Nanos::from_secs(1));
        assert_eq!(c.chunk_rows(1920), 1, "a slow copy still makes progress");
    }
}
