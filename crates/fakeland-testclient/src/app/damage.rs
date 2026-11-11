//! Surface damage of a frame: the exact bounding box of the pixels that differ from the last
//! committed frame, or the whole buffer.

use fakeland_types::{Physical, Rect, Size};
use serde::Serialize;

use crate::draw::changed_bbox;

/// How the client reports damage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DamageMode {
    /// The bounding box of the changed pixels; no damage for an unchanged frame.
    #[default]
    Exact,
    /// The whole buffer on every frame.
    Full,
}

/// The last committed frame.
#[derive(Clone, Copy, Debug)]
pub struct Previous<'a> {
    /// Its pixels.
    pub pixels: &'a [u8],
    /// Its buffer size.
    pub size: Size<Physical>,
}

/// Returns the buffer damage of a new frame, or `None` when nothing changed.
pub fn frame_damage(
    mode: DamageMode,
    previous: Option<Previous<'_>>,
    next: &[u8],
    size: Size<Physical>,
) -> Option<Rect<Physical>> {
    let full = Rect::from_size(size);
    match (mode, previous) {
        (DamageMode::Exact, Some(p)) if p.size == size => changed_bbox(p.pixels, next, size),
        _ => Some(full),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_damage_tracks_changes() {
        let size = Size::new(4, 4);
        let a = vec![0u8; 64];
        let mut b = a.clone();
        b[4 * (4 + 1)] = 9;
        let prev = Previous { pixels: &a, size };
        assert_eq!(
            frame_damage(DamageMode::Exact, Some(prev), &b, size),
            Some(Rect::new(1, 1, 1, 1))
        );
        assert_eq!(frame_damage(DamageMode::Exact, Some(prev), &a, size), None);
    }

    #[test]
    fn full_on_first_frame_resize_or_full_mode() {
        let size = Size::new(4, 4);
        let a = vec![0u8; 64];
        let full = Some(Rect::new(0, 0, 4, 4));
        assert_eq!(frame_damage(DamageMode::Exact, None, &a, size), full);
        let small = vec![0u8; 16];
        let prev = Previous {
            pixels: &small,
            size: Size::new(2, 2),
        };
        assert_eq!(frame_damage(DamageMode::Exact, Some(prev), &a, size), full);
        let prev = Previous { pixels: &a, size };
        assert_eq!(frame_damage(DamageMode::Full, Some(prev), &a, size), full);
    }
}
