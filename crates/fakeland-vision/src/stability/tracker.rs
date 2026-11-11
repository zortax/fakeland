//! A tracker of successive frames that reports when content stays equal for a number of frames.

use fakeland_types::{Physical, Rect};
use serde::{Deserialize, Serialize};

use super::region_hash;
use crate::diff::count_changed_up_to;
use crate::image::{BPP, ImageView};

/// Rules for frame equality and stability.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StabilityConfig {
    /// Consecutive equal frames that make content stable. One means every frame is stable.
    pub frames: u32,
    /// A pixel changes when its largest color channel difference exceeds this value.
    pub threshold: u8,
    /// Two frames are equal when at most this fraction of the region's pixels change.
    pub max_changed_fraction: f64,
    /// The region to watch. `None` watches the whole frame.
    pub roi: Option<Rect<Physical>>,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            frames: 3,
            threshold: 0,
            max_changed_fraction: 0.0,
            roi: None,
        }
    }
}

/// The state after one observed frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StabilityStatus {
    /// The run of consecutive equal frames that ends with this frame, this frame included.
    pub run: u32,
    /// True when the run reaches the configured frame count.
    pub stable: bool,
    /// True when this frame differs from the previous frame, or no previous frame exists.
    pub changed: bool,
}

/// The last observed sample.
#[derive(Debug)]
enum Last {
    None,
    Hash(u64),
    Frame { width: u32, height: u32 },
}

/// Tracks successive frames or content hashes and reports stability.
///
/// Exact rules (threshold zero, no changed fraction) compare region hashes and keep no pixels.
/// Tolerant rules keep a copy of the last region and count changed pixels with early exit.
#[derive(Debug)]
pub struct StabilityTracker {
    config: StabilityConfig,
    last: Last,
    pixels: Vec<u8>,
    run: u32,
}

impl StabilityTracker {
    /// Builds a tracker with no observed frame.
    pub fn new(config: StabilityConfig) -> Self {
        Self {
            config,
            last: Last::None,
            pixels: Vec::new(),
            run: 0,
        }
    }

    /// Returns the rules.
    pub fn config(&self) -> &StabilityConfig {
        &self.config
    }

    /// Forgets every observed frame.
    pub fn reset(&mut self) {
        self.last = Last::None;
        self.run = 0;
    }

    /// Returns the current run of equal frames.
    pub fn run(&self) -> u32 {
        self.run
    }

    /// Returns true when the current run reaches the configured frame count.
    pub fn is_stable(&self) -> bool {
        self.run >= self.config.frames.max(1)
    }

    /// Observes a content hash, for example from [`region_hash`]. Hashes compare exactly.
    pub fn push_hash(&mut self, hash: u64) -> StabilityStatus {
        let equal = matches!(self.last, Last::Hash(h) if h == hash);
        self.last = Last::Hash(hash);
        self.advance(equal)
    }

    /// Observes a frame. The watched region is clipped to the frame. A frame with no overlap
    /// counts as a change.
    pub fn push_frame<'a>(&mut self, frame: impl Into<ImageView<'a>>) -> StabilityStatus {
        let frame = frame.into();
        let Some(view) = frame.clip(self.config.roi).and_then(|r| frame.region(r)) else {
            self.last = Last::None;
            return self.advance(false);
        };
        if self.exact() {
            return self.push_hash(region_hash(view, None));
        }
        let equal = match self.last {
            Last::Frame { width, height } if width == view.width() && height == view.height() => {
                let stored =
                    ImageView::from_parts(width, height, width as usize * BPP, &self.pixels);
                let area = width as u64 * height as u64;
                let allowed =
                    (self.config.max_changed_fraction.clamp(0.0, 1.0) * area as f64).floor() as u64;
                count_changed_up_to(&stored, &view, self.config.threshold, allowed + 1) <= allowed
            }
            _ => false,
        };
        self.pixels.clear();
        for row in view.rows() {
            self.pixels.extend_from_slice(row);
        }
        self.last = Last::Frame {
            width: view.width(),
            height: view.height(),
        };
        self.advance(equal)
    }

    fn exact(&self) -> bool {
        self.config.threshold == 0 && self.config.max_changed_fraction <= 0.0
    }

    fn advance(&mut self, equal: bool) -> StabilityStatus {
        self.run = if equal { self.run.saturating_add(1) } else { 1 };
        StabilityStatus {
            run: self.run,
            stable: self.is_stable(),
            changed: !equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    #[test]
    fn exact_rules_need_n_equal_frames() {
        let mut t = StabilityTracker::new(StabilityConfig {
            frames: 3,
            ..Default::default()
        });
        let a = Image::filled(10, 10, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.set_pixel(5, 5, [1, 0, 0, 255]);
        assert_eq!(
            t.push_frame(&a),
            StabilityStatus {
                run: 1,
                stable: false,
                changed: true
            }
        );
        assert_eq!(t.push_frame(&a).run, 2);
        let s = t.push_frame(&a);
        assert!(s.stable && !s.changed && s.run == 3);
        let s = t.push_frame(&b);
        assert!(!s.stable && s.changed && s.run == 1);
        t.push_frame(&b);
        assert!(t.push_frame(&b).stable);
    }

    #[test]
    fn tolerant_rules_ignore_small_change() {
        let mut t = StabilityTracker::new(StabilityConfig {
            frames: 2,
            threshold: 4,
            max_changed_fraction: 0.02,
            roi: None,
        });
        let a = Image::filled(10, 10, [100, 100, 100, 255]);
        let noisy = Image::filled(10, 10, [103, 98, 100, 255]);
        let mut spots = a.clone();
        spots.set_pixel(1, 1, [255, 0, 0, 255]);
        spots.set_pixel(2, 1, [255, 0, 0, 255]);
        let mut more = spots.clone();
        more.set_pixel(3, 1, [255, 0, 0, 255]);
        t.push_frame(&a);
        assert!(t.push_frame(&noisy).stable);
        assert!(t.push_frame(&spots).stable);
        assert!(t.push_frame(&a).stable);
        assert!(!t.push_frame(&more).stable);
    }

    #[test]
    fn roi_ignores_outside_change() {
        let mut t = StabilityTracker::new(StabilityConfig {
            frames: 2,
            roi: Some(Rect::new(0, 0, 5, 5)),
            ..Default::default()
        });
        let a = Image::filled(10, 10, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.set_pixel(8, 8, [255, 255, 255, 255]);
        t.push_frame(&a);
        assert!(t.push_frame(&b).stable);
    }

    #[test]
    fn hashes_and_reset() {
        let mut t = StabilityTracker::new(StabilityConfig {
            frames: 2,
            ..Default::default()
        });
        t.push_hash(7);
        assert!(t.push_hash(7).stable);
        t.reset();
        assert_eq!(t.run(), 0);
        assert!(!t.push_hash(7).stable);
        assert!(!t.push_hash(8).stable);
    }
}
