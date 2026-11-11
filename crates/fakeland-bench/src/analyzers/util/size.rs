//! The logical size of a buffer and its fit to an output scale.

use fakeland_trace::BufferInfo;

/// Returns the logical size of a buffer: its viewport destination, else its pixel size over its
/// buffer scale.
pub fn logical_size(b: &BufferInfo) -> (f64, f64) {
    match b.viewport {
        Some(v) => (f64::from(v.w), f64::from(v.h)),
        None => {
            let s = if b.scale > 0.0 { b.scale } else { 1.0 };
            (f64::from(b.width) / s, f64::from(b.height) / s)
        }
    }
}

/// Returns true when the buffer has the pixel size its logical size needs at an output scale,
/// within one pixel per axis.
pub fn physical_size_matches(b: &BufferInfo, output_scale: f64) -> bool {
    let (w, h) = logical_size(b);
    (f64::from(b.width) - w * output_scale).abs() <= 1.0
        && (f64::from(b.height) - h * output_scale).abs() <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::BufferKind;
    use fakeland_types::{BufferId, Size};

    fn buf(w: i32, h: i32, scale: f64, viewport: Option<(i32, i32)>) -> BufferInfo {
        BufferInfo {
            id: BufferId(1),
            kind: BufferKind::Shm,
            format: 0,
            modifier: None,
            width: w,
            height: h,
            scale,
            viewport: viewport.map(|(w, h)| Size::new(w, h)),
        }
    }

    #[test]
    fn sizes() {
        assert_eq!(logical_size(&buf(1920, 1080, 2.0, None)), (960.0, 540.0));
        assert_eq!(
            logical_size(&buf(1920, 1080, 1.0, Some((1280, 720)))),
            (1280.0, 720.0)
        );
        assert!(physical_size_matches(
            &buf(1920, 1080, 1.0, Some((1280, 720))),
            1.5
        ));
        assert!(!physical_size_matches(&buf(1280, 720, 1.0, None), 1.5));
        assert!(physical_size_matches(&buf(2560, 1440, 2.0, None), 2.0));
        assert!(!physical_size_matches(&buf(2560, 1440, 2.0, None), 1.5));
    }
}
