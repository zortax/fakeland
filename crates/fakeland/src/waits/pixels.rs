//! Pixel conditions on a captured region. Coordinates of a condition are logical and relative to
//! the region; `scale` converts them to image pixels.

use base64::Engine;
use fakeland_scenario::PixelCondition;
use fakeland_vision::{Image, ImageStore, NoImages, Predicate, Rgba, diff, evaluate};
use thiserror::Error;

/// The per-pixel channel delta above which a pixel counts as changed.
pub const PIXEL_DELTA: u8 = 2;

/// A condition that cannot be evaluated.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ConditionError {
    /// The reference image is not valid base64 PNG.
    #[error("the reference image does not decode: {0}")]
    Reference(String),
    /// The reference image and the region differ in size.
    #[error("the reference is {ref_w}x{ref_h} and the region is {w}x{h}")]
    ReferenceSize {
        /// Reference width.
        ref_w: u32,
        /// Reference height.
        ref_h: u32,
        /// Region width.
        w: u32,
        /// Region height.
        h: u32,
    },
    /// The image analysis failed.
    #[error("{0}")]
    Vision(String),
}

/// A compiled condition: the reference image is decoded once.
#[derive(Debug, Clone)]
pub struct Condition {
    condition: PixelCondition,
    reference: Option<Image>,
}

impl Condition {
    /// Compiles a condition.
    pub fn new(condition: &PixelCondition) -> Result<Self, ConditionError> {
        let reference = match condition {
            PixelCondition::MatchesImage { png_base64, .. } => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(png_base64.trim())
                    .map_err(|e| ConditionError::Reference(e.to_string()))?;
                Some(
                    Image::decode_png(&bytes)
                        .map_err(|e| ConditionError::Reference(e.to_string()))?,
                )
            }
            _ => None,
        };
        Ok(Self {
            condition: condition.clone(),
            reference,
        })
    }

    /// Returns true when the condition needs a baseline image.
    pub fn needs_baseline(&self) -> bool {
        matches!(self.condition, PixelCondition::Changed { .. })
    }

    /// Evaluates the condition on a region image. `scale` is image pixels per logical unit.
    pub fn matches(
        &self,
        image: &Image,
        baseline: Option<&Image>,
        scale: f64,
    ) -> Result<bool, ConditionError> {
        let vision = |e: &dyn std::fmt::Display| ConditionError::Vision(e.to_string());
        match &self.condition {
            PixelCondition::Changed { threshold } => {
                let Some(base) = baseline else {
                    return Ok(false);
                };
                Ok(changed_fraction(image, base).map_err(|e| vision(&e))? > *threshold)
            }
            PixelCondition::ColorAt {
                x,
                y,
                rgba,
                tolerance,
            } => {
                let px = ((*x as f64 + 0.5) * scale).floor().max(0.0) as u32;
                let py = ((*y as f64 + 0.5) * scale).floor().max(0.0) as u32;
                let p = Predicate::ColorAt {
                    x: px.min(image.width().saturating_sub(1)),
                    y: py.min(image.height().saturating_sub(1)),
                    rgba: Rgba(rgba.0),
                    tolerance: *tolerance,
                };
                Ok(evaluate(image, &p, &NoImages)
                    .map_err(|e| vision(&e))?
                    .matched)
            }
            PixelCondition::MeanColor { rgba, tolerance } => {
                let p = Predicate::MeanColor {
                    roi: None,
                    rgba: Rgba(rgba.0),
                    tolerance: *tolerance,
                };
                Ok(evaluate(image, &p, &NoImages)
                    .map_err(|e| vision(&e))?
                    .matched)
            }
            PixelCondition::MatchesImage { max_mean_delta, .. } => {
                let reference = self
                    .reference
                    .as_ref()
                    .ok_or_else(|| ConditionError::Reference("missing".into()))?;
                let fitted = fit_to(image, reference)?;
                let mut store = ImageStore::new();
                let handle = store.insert(reference.clone());
                let p = Predicate::MatchesImage {
                    reference: handle,
                    roi: None,
                    max_mean_delta: *max_mean_delta,
                };
                Ok(evaluate(&fitted, &p, &store)
                    .map_err(|e| vision(&e))?
                    .matched)
            }
        }
    }
}

/// Returns the fraction of pixels that changed between two images. Images of different sizes
/// count as fully changed.
pub fn changed_fraction(a: &Image, b: &Image) -> Result<f64, fakeland_vision::DiffError> {
    if a.width() != b.width() || a.height() != b.height() {
        return Ok(1.0);
    }
    Ok(diff(a, b, None, PIXEL_DELTA)?.changed_fraction)
}

/// Scales a region image down to the size of a reference when the region is larger.
fn fit_to(image: &Image, reference: &Image) -> Result<Image, ConditionError> {
    if image.width() == reference.width() && image.height() == reference.height() {
        return Ok(image.clone());
    }
    let max = reference.width().max(reference.height());
    let (scaled, _) = image.downscale_to_max_dim(max);
    if scaled.width() == reference.width() && scaled.height() == reference.height() {
        return Ok(scaled);
    }
    Err(ConditionError::ReferenceSize {
        ref_w: reference.width(),
        ref_h: reference.height(),
        w: image.width(),
        h: image.height(),
    })
}

#[cfg(test)]
mod tests {
    use fakeland_scenario::Rgba as SRgba;
    use fakeland_types::Rect;

    use super::*;

    #[test]
    fn color_mean_and_change() {
        let mut img = Image::filled(40, 20, [10, 20, 30, 255]);
        img.fill_rect(Rect::new(20, 0, 20, 20), [200, 0, 0, 255]);
        let at = Condition::new(&PixelCondition::ColorAt {
            x: 15,
            y: 5,
            rgba: SRgba::rgb(200, 0, 0),
            tolerance: 8,
        })
        .unwrap();
        assert!(at.matches(&img, None, 2.0).unwrap());
        assert!(!at.matches(&img, None, 1.0).unwrap());
        let mean = Condition::new(&PixelCondition::MeanColor {
            rgba: SRgba::rgb(105, 10, 15),
            tolerance: 8,
        })
        .unwrap();
        assert!(mean.matches(&img, None, 1.0).unwrap());
        let changed = Condition::new(&PixelCondition::Changed { threshold: 0.4 }).unwrap();
        let base = Image::filled(40, 20, [10, 20, 30, 255]);
        assert!(changed.needs_baseline());
        assert!(changed.matches(&img, Some(&base), 1.0).unwrap());
        assert!(!changed.matches(&base, Some(&base), 1.0).unwrap());
        assert_eq!(changed_fraction(&img, &Image::new(3, 3)).unwrap(), 1.0);
    }

    #[test]
    fn reference_images_match_after_downscale() {
        let reference = Image::filled(10, 5, [50, 60, 70, 255]);
        let png = reference.encode_png().unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let cond = Condition::new(&PixelCondition::MatchesImage {
            png_base64: b64,
            max_mean_delta: 2.0,
        })
        .unwrap();
        assert!(
            cond.matches(&Image::filled(20, 10, [50, 60, 70, 255]), None, 2.0)
                .unwrap()
        );
        assert!(
            !cond
                .matches(&Image::filled(10, 5, [0, 0, 0, 255]), None, 1.0)
                .unwrap()
        );
        assert!(matches!(
            cond.matches(&Image::filled(7, 7, [0, 0, 0, 255]), None, 1.0),
            Err(ConditionError::ReferenceSize { .. })
        ));
        assert!(
            Condition::new(&PixelCondition::MatchesImage {
                png_base64: "!!".into(),
                max_mean_delta: 1.0
            })
            .is_err()
        );
    }
}
