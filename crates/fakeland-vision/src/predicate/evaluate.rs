//! Evaluation of predicates on a frame.

use fakeland_types::{Physical, Rect};
use serde::{Deserialize, Serialize};

use super::{ImageHandle, ImageSource, Predicate, PredicateError};
use crate::diff::{DiffStats, diff};
use crate::image::{BPP, ImageView};

/// The result of a predicate on a frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    /// True when the predicate holds.
    pub matched: bool,
    /// How strongly the predicate holds, from 0 to 1. A color or image match scores one minus the
    /// difference over 255; a change scores the changed fraction. `All` takes the minimum, `Any`
    /// the maximum, and `Not` one minus the inner score.
    pub score: f64,
}

/// Evaluates a predicate on a frame. `images` resolves the handles of baseline and reference
/// images.
pub fn evaluate<'a>(
    frame: impl Into<ImageView<'a>>,
    predicate: &Predicate,
    images: &dyn ImageSource,
) -> Result<Evaluation, PredicateError> {
    eval(&frame.into(), predicate, images)
}

fn eval(
    frame: &ImageView<'_>,
    predicate: &Predicate,
    images: &dyn ImageSource,
) -> Result<Evaluation, PredicateError> {
    match predicate {
        Predicate::ColorAt {
            x,
            y,
            rgba,
            tolerance,
        } => {
            let px = frame
                .pixel(*x, *y)
                .ok_or(PredicateError::OutsideFrame { x: *x, y: *y })?;
            let d = (0..4).map(|c| px[c].abs_diff(rgba.0[c])).max().unwrap_or(0);
            Ok(color_result(d as f64, *tolerance))
        }
        Predicate::MeanColor {
            roi,
            rgba,
            tolerance,
        } => {
            let mean = mean_color(frame, *roi)?;
            let d = (0..4)
                .map(|c| (mean[c] - rgba.0[c] as f64).abs())
                .fold(0.0, f64::max);
            Ok(color_result(d, *tolerance))
        }
        Predicate::DiffersFrom {
            baseline,
            roi,
            threshold,
            min_changed_fraction,
        } => {
            let stats = compare(frame, *baseline, *roi, *threshold, images)?;
            Ok(Evaluation {
                matched: stats.changed_pixels > 0
                    && stats.changed_fraction >= *min_changed_fraction,
                score: stats.changed_fraction,
            })
        }
        Predicate::MatchesImage {
            reference,
            roi,
            max_mean_delta,
        } => {
            let stats = compare(frame, *reference, *roi, 0, images)?;
            Ok(Evaluation {
                matched: stats.mean_delta <= *max_mean_delta,
                score: 1.0 - stats.mean_delta / 255.0,
            })
        }
        Predicate::All { of } => {
            let mut out = Evaluation {
                matched: true,
                score: 1.0,
            };
            for p in of {
                let e = eval(frame, p, images)?;
                out.matched &= e.matched;
                out.score = out.score.min(e.score);
            }
            Ok(out)
        }
        Predicate::Any { of } => {
            let mut out = Evaluation {
                matched: false,
                score: 0.0,
            };
            for p in of {
                let e = eval(frame, p, images)?;
                out.matched |= e.matched;
                out.score = out.score.max(e.score);
            }
            Ok(out)
        }
        Predicate::Not { predicate } => {
            let e = eval(frame, predicate, images)?;
            Ok(Evaluation {
                matched: !e.matched,
                score: 1.0 - e.score,
            })
        }
    }
}

fn color_result(delta: f64, tolerance: u8) -> Evaluation {
    Evaluation {
        matched: delta <= tolerance as f64,
        score: 1.0 - delta / 255.0,
    }
}

/// Returns the mean of each channel over a region.
fn mean_color(
    frame: &ImageView<'_>,
    roi: Option<Rect<Physical>>,
) -> Result<[f64; 4], PredicateError> {
    let view = frame
        .clip(roi)
        .and_then(|r| frame.region(r))
        .ok_or(PredicateError::EmptyRegion(roi))?;
    let mut sums = [0u64; 4];
    for row in view.rows() {
        let mut row_sums = [0u32; 4];
        for px in row.chunks_exact(BPP) {
            for (s, &v) in row_sums.iter_mut().zip(px) {
                *s += v as u32;
            }
        }
        for (s, r) in sums.iter_mut().zip(row_sums) {
            *s += r as u64;
        }
    }
    let n = view.width() as f64 * view.height() as f64;
    Ok(sums.map(|s| s as f64 / n))
}

/// Compares a frame region with the matching part of a stored image. A stored image of the frame
/// size compares the same region; a stored image of the clipped region size aligns with the region
/// origin.
fn compare(
    frame: &ImageView<'_>,
    handle: ImageHandle,
    roi: Option<Rect<Physical>>,
    threshold: u8,
    images: &dyn ImageSource,
) -> Result<DiffStats, PredicateError> {
    let other = images
        .image(handle)
        .ok_or(PredicateError::UnknownImage(handle))?;
    let r = frame.clip(roi).ok_or(PredicateError::EmptyRegion(roi))?;
    if other.same_size(frame) {
        return Ok(diff(*frame, other, Some(r), threshold)?);
    }
    if other.width() == r.w as u32 && other.height() == r.h as u32 {
        let region = frame.region(r).ok_or(PredicateError::EmptyRegion(roi))?;
        return Ok(diff(region, other, None, threshold)?);
    }
    Err(PredicateError::ReferenceSize {
        width: other.width(),
        height: other.height(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::predicate::{ImageStore, NoImages, Rgba};

    fn frame() -> Image {
        let mut img = Image::filled(40, 30, [10, 20, 30, 255]);
        img.fill_rect(Rect::new(10, 10, 10, 10), [200, 0, 0, 255]);
        img
    }

    #[test]
    fn color_at() {
        let f = frame();
        let p = Predicate::ColorAt {
            x: 12,
            y: 12,
            rgba: Rgba::opaque(196, 0, 0),
            tolerance: 4,
        };
        let e = evaluate(&f, &p, &NoImages).unwrap();
        assert!(e.matched);
        assert!((e.score - (1.0 - 4.0 / 255.0)).abs() < 1e-12);
        let p = Predicate::ColorAt {
            x: 12,
            y: 12,
            rgba: Rgba::opaque(190, 0, 0),
            tolerance: 4,
        };
        assert!(!evaluate(&f, &p, &NoImages).unwrap().matched);
        let p = Predicate::ColorAt {
            x: 40,
            y: 0,
            rgba: Rgba::opaque(0, 0, 0),
            tolerance: 0,
        };
        assert_eq!(
            evaluate(&f, &p, &NoImages),
            Err(PredicateError::OutsideFrame { x: 40, y: 0 })
        );
    }

    #[test]
    fn mean_color() {
        let f = frame();
        // Half of the region is red and half is the background.
        let p = Predicate::MeanColor {
            roi: Some(Rect::new(10, 10, 20, 10)),
            rgba: Rgba::opaque(105, 10, 15),
            tolerance: 1,
        };
        assert!(evaluate(&f, &p, &NoImages).unwrap().matched);
        let p = Predicate::MeanColor {
            roi: Some(Rect::new(100, 100, 5, 5)),
            rgba: Rgba::opaque(0, 0, 0),
            tolerance: 1,
        };
        assert!(matches!(
            evaluate(&f, &p, &NoImages),
            Err(PredicateError::EmptyRegion(_))
        ));
    }

    #[test]
    fn differs_and_matches() {
        let mut store = ImageStore::new();
        let base = store.insert(Image::filled(40, 30, [10, 20, 30, 255]));
        let crop = store.insert(frame().crop(Rect::new(10, 10, 10, 10)).unwrap());
        let f = frame();
        let differs = |roi, min| Predicate::DiffersFrom {
            baseline: base,
            roi,
            threshold: 10,
            min_changed_fraction: min,
        };
        let e = evaluate(&f, &differs(None, 0.0), &store).unwrap();
        assert!(e.matched);
        assert!((e.score - 100.0 / 1200.0).abs() < 1e-12);
        assert!(!evaluate(&f, &differs(None, 0.5), &store).unwrap().matched);
        let outside = Some(Rect::new(0, 0, 10, 10));
        assert!(
            !evaluate(&f, &differs(outside, 0.0), &store)
                .unwrap()
                .matched
        );

        let matches = |reference, roi| Predicate::MatchesImage {
            reference,
            roi,
            max_mean_delta: 0.5,
        };
        let region = Some(Rect::new(10, 10, 10, 10));
        assert!(
            evaluate(&f, &matches(crop, region), &store)
                .unwrap()
                .matched
        );
        assert!(
            evaluate(&f, &matches(base, outside), &store)
                .unwrap()
                .matched
        );
        assert!(!evaluate(&f, &matches(base, None), &store).unwrap().matched);
        assert!(matches!(
            evaluate(&f, &matches(crop, None), &store),
            Err(PredicateError::ReferenceSize { .. })
        ));
        assert_eq!(
            evaluate(&f, &matches(ImageHandle(99), None), &store),
            Err(PredicateError::UnknownImage(ImageHandle(99)))
        );
    }

    #[test]
    fn combinators() {
        let f = frame();
        let yes = Predicate::ColorAt {
            x: 0,
            y: 0,
            rgba: Rgba::opaque(10, 20, 30),
            tolerance: 0,
        };
        let no = Predicate::ColorAt {
            x: 0,
            y: 0,
            rgba: Rgba::opaque(10, 20, 80),
            tolerance: 0,
        };
        let all = Predicate::All {
            of: vec![yes.clone(), no.clone()],
        };
        let any = Predicate::Any {
            of: vec![yes.clone(), no.clone()],
        };
        let not = Predicate::Not {
            predicate: Box::new(no),
        };
        assert!(!evaluate(&f, &all, &NoImages).unwrap().matched);
        assert!(evaluate(&f, &any, &NoImages).unwrap().matched);
        let e = evaluate(&f, &not, &NoImages).unwrap();
        assert!(e.matched);
        assert!((e.score - 50.0 / 255.0).abs() < 1e-12);
        assert!(
            evaluate(&f, &Predicate::All { of: vec![] }, &NoImages)
                .unwrap()
                .matched
        );
    }
}
