//! A fast 64-bit content hash of a frame region. Alpha does not count, so the hash agrees with the
//! change measure of the diff module at threshold zero.

use fakeland_types::{Physical, Rect};

use crate::image::ImageView;

/// Multiplier of the lane mix.
const K: u64 = 0x9E37_79B9_7F4A_7C15;
/// Clears the alpha byte of both pixels in a little-endian word.
const NO_ALPHA: u64 = 0x00FF_FFFF_00FF_FFFF;

/// Finalizes a 64-bit state with the splitmix64 mix.
fn finish(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline(always)]
fn mix(lane: u64, word: u64) -> u64 {
    (lane ^ (word & NO_ALPHA)).wrapping_mul(K).rotate_left(31)
}

#[inline(always)]
fn word(bytes: &[u8]) -> u64 {
    let mut w = [0u8; 8];
    w[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(w)
}

/// Returns a 64-bit hash of the RGB content of a region. `None` hashes the whole frame. The region
/// is clipped to the frame; the hash covers the clipped size and its pixels. Equal content gives
/// equal hashes; a different hash means different content.
pub fn region_hash<'a>(frame: impl Into<ImageView<'a>>, roi: Option<Rect<Physical>>) -> u64 {
    let frame = frame.into();
    let Some(view) = frame.clip(roi).and_then(|r| frame.region(r)) else {
        return finish(0);
    };
    let mut lanes = [
        K ^ view.width() as u64,
        K.rotate_left(17) ^ view.height() as u64,
        K.rotate_left(34),
        K.rotate_left(51),
    ];
    for row in view.rows() {
        let mut blocks = row.chunks_exact(32);
        for block in &mut blocks {
            for (i, lane) in lanes.iter_mut().enumerate() {
                *lane = mix(*lane, word(&block[i * 8..i * 8 + 8]));
            }
        }
        for (i, chunk) in blocks.remainder().chunks(8).enumerate() {
            lanes[i] = mix(lanes[i], word(chunk));
        }
        lanes[0] = mix(lanes[0], 0x5bd1_e995);
    }
    finish(
        lanes[0] ^ lanes[1].rotate_left(13) ^ lanes[2].rotate_left(29) ^ lanes[3].rotate_left(47),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    #[test]
    fn equal_content_equal_hash() {
        let a = Image::filled(33, 7, [1, 2, 3, 255]);
        let mut b = Image::filled(33, 7, [1, 2, 3, 0]);
        assert_eq!(region_hash(&a, None), region_hash(&b, None));
        b.set_pixel(32, 6, [1, 2, 4, 0]);
        assert_ne!(region_hash(&a, None), region_hash(&b, None));
        assert_eq!(
            region_hash(&a, Some(Rect::new(0, 0, 32, 7))),
            region_hash(&b, Some(Rect::new(0, 0, 32, 7)))
        );
    }

    #[test]
    fn size_and_row_layout_count() {
        let a = Image::filled(8, 2, [9, 9, 9, 255]);
        let b = Image::filled(4, 4, [9, 9, 9, 255]);
        assert_ne!(region_hash(&a, None), region_hash(&b, None));
        let mut c = Image::filled(16, 16, [0, 0, 0, 255]);
        c.set_pixel(3, 3, [255, 0, 0, 255]);
        let mut d = Image::filled(16, 16, [0, 0, 0, 255]);
        d.set_pixel(4, 3, [255, 0, 0, 255]);
        assert_ne!(region_hash(&c, None), region_hash(&d, None));
    }

    #[test]
    fn region_matches_crop() {
        let mut a = Image::filled(50, 40, [3, 3, 3, 255]);
        a.fill_rect(Rect::new(10, 10, 5, 5), [200, 1, 1, 255]);
        let r = Rect::new(8, 9, 20, 12);
        let crop = a.crop(r).unwrap();
        assert_eq!(region_hash(&a, Some(r)), region_hash(&crop, None));
    }
}
