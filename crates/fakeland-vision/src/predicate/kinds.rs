//! The predicate kinds and the RGBA color value.

use fakeland_types::{Physical, Rect};
use serde::{Deserialize, Serialize};

use super::ImageHandle;

/// An RGBA8 color. It serializes as an array `[r, g, b, a]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    /// Builds a color.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// Builds an opaque color.
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }
}

impl From<[u8; 4]> for Rgba {
    fn from(px: [u8; 4]) -> Self {
        Self(px)
    }
}

impl From<Rgba> for [u8; 4] {
    fn from(c: Rgba) -> Self {
        c.0
    }
}

/// A test on the pixels of a frame. Coordinates are frame pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Predicate {
    /// The pixel at a point has a color. Every channel, alpha included, lies within the tolerance.
    ColorAt {
        /// Horizontal coordinate.
        x: u32,
        /// Vertical coordinate.
        y: u32,
        /// The expected color.
        rgba: Rgba,
        /// The largest allowed channel difference.
        tolerance: u8,
    },
    /// The mean color of a region has a color. Every channel mean, alpha included, lies within the
    /// tolerance.
    MeanColor {
        /// The region. `None` selects the whole frame.
        #[serde(default)]
        roi: Option<Rect<Physical>>,
        /// The expected mean color.
        rgba: Rgba,
        /// The largest allowed difference of a channel mean.
        tolerance: u8,
    },
    /// The frame differs from a baseline image in a region.
    DiffersFrom {
        /// The baseline image.
        baseline: ImageHandle,
        /// The region. `None` selects the whole frame.
        #[serde(default)]
        roi: Option<Rect<Physical>>,
        /// A pixel changes when its largest color channel difference exceeds this value.
        #[serde(default)]
        threshold: u8,
        /// The smallest fraction of changed pixels in the region. Zero means one pixel suffices.
        #[serde(default)]
        min_changed_fraction: f64,
    },
    /// The frame matches a reference image in a region.
    MatchesImage {
        /// The reference image.
        reference: ImageHandle,
        /// The region. `None` selects the whole frame.
        #[serde(default)]
        roi: Option<Rect<Physical>>,
        /// The largest allowed mean of the per-pixel color difference.
        max_mean_delta: f64,
    },
    /// Every inner predicate holds.
    All {
        /// The inner predicates.
        of: Vec<Predicate>,
    },
    /// At least one inner predicate holds.
    Any {
        /// The inner predicates.
        of: Vec<Predicate>,
    },
    /// The inner predicate does not hold.
    Not {
        /// The inner predicate.
        predicate: Box<Predicate>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let p = Predicate::All {
            of: vec![
                Predicate::ColorAt {
                    x: 3,
                    y: 4,
                    rgba: Rgba::opaque(1, 2, 3),
                    tolerance: 2,
                },
                Predicate::Not {
                    predicate: Box::new(Predicate::DiffersFrom {
                        baseline: ImageHandle(9),
                        roi: Some(Rect::new(1, 2, 3, 4)),
                        threshold: 8,
                        min_changed_fraction: 0.5,
                    }),
                },
            ],
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""kind":"color_at""#));
        assert!(json.contains(r#""rgba":[1,2,3,255]"#));
        let back: Predicate = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn defaults_fill_optional_fields() {
        let p: Predicate = serde_json::from_str(r#"{"kind":"differs_from","baseline":2}"#).unwrap();
        assert_eq!(
            p,
            Predicate::DiffersFrom {
                baseline: ImageHandle(2),
                roi: None,
                threshold: 0,
                min_changed_fraction: 0.0
            }
        );
    }
}
