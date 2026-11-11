//! Maps buffer damage into the pixel space of a window capture.

use fakeland::CapturedFrame;
use fakeland_trace::BufferInfo;
use fakeland_types::{Physical, Rect};

use crate::analyzers::util::logical_size;

/// Maps damage rectangles from buffer pixels into the pixels of a capture of the whole window.
/// The surface origin is the capture origin, which holds for whole-window captures of clients
/// whose window geometry starts at the surface origin.
pub fn damage_in_capture(
    damage: &[Rect<Physical>],
    buffer: &BufferInfo,
    frame: &CapturedFrame,
) -> Vec<Rect<Physical>> {
    let (lw, lh) = logical_size(buffer);
    if buffer.width <= 0 || buffer.height <= 0 || frame.scale <= 0.0 {
        return Vec::new();
    }
    let sx = lw / f64::from(buffer.width) / frame.scale;
    let sy = lh / f64::from(buffer.height) / frame.scale;
    damage
        .iter()
        .map(|r| {
            let x0 = (f64::from(r.x) * sx).floor();
            let y0 = (f64::from(r.y) * sy).floor();
            let x1 = (f64::from(r.x.saturating_add(r.w)) * sx).ceil();
            let y1 = (f64::from(r.y.saturating_add(r.h)) * sy).ceil();
            let clamp = |v: f64| v.clamp(-1e9, 1e9) as i32;
            Rect::new(
                clamp(x0),
                clamp(y0),
                clamp(x1 - x0).max(0),
                clamp(y1 - y0).max(0),
            )
        })
        .filter(|r| !r.is_empty())
        .collect()
}
