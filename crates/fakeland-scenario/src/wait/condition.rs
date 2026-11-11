//! Pixel conditions on a region of interest.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Rgba;

fn default_threshold() -> f64 {
    0.001
}

fn default_tolerance() -> u8 {
    8
}

fn default_mean_delta() -> f64 {
    2.0
}

/// A condition on the pixels of a region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PixelCondition {
    /// The region differs from its contents when the wait or probe started.
    Changed {
        /// The fraction of region pixels that must change, in `0..=1`.
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
    /// The pixel at region-relative logical coordinates has a color.
    ColorAt {
        /// Horizontal coordinate in the region.
        x: u32,
        /// Vertical coordinate in the region.
        y: u32,
        /// The color.
        rgba: Rgba,
        /// The largest difference per channel.
        #[serde(default = "default_tolerance")]
        tolerance: u8,
    },
    /// The mean color of the region.
    MeanColor {
        /// The color.
        rgba: Rgba,
        /// The largest difference per channel.
        #[serde(default = "default_tolerance")]
        tolerance: u8,
    },
    /// The region matches a reference image.
    MatchesImage {
        /// The PNG file, base64 encoded.
        png_base64: String,
        /// The largest mean difference per channel.
        #[serde(default = "default_mean_delta")]
        max_mean_delta: f64,
    },
}

impl PixelCondition {
    /// Any change at the default threshold.
    pub fn changed() -> Self {
        Self::Changed {
            threshold: default_threshold(),
        }
    }
}
