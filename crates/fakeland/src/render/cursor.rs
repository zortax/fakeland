//! The built-in cursor image: a black arrow with a white outline, drawn from a bitmap.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::Transform;

/// The arrow bitmap: `#` is the outline, `.` is the fill, a space is transparent.
const ARROW: &[&str] = &[
    "#           ",
    "##          ",
    "#.#         ",
    "#..#        ",
    "#...#       ",
    "#....#      ",
    "#.....#     ",
    "#......#    ",
    "#.......#   ",
    "#........#  ",
    "#.....#####",
    "#..#..#     ",
    "#.# #..#    ",
    "##  #..#    ",
    "#    #..#   ",
    "     #..#   ",
    "      ##    ",
];

/// Returns the arrow as packed ARGB8888 pixels in memory order, with its width and height.
pub fn arrow_pixels() -> (Vec<u8>, i32, i32) {
    let w = ARROW.iter().map(|r| r.len()).max().unwrap_or(1);
    let h = ARROW.len();
    let mut out = vec![0u8; w * h * 4];
    for (y, row) in ARROW.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let px: [u8; 4] = match c {
                '#' => [255, 255, 255, 255],
                '.' => [0, 0, 0, 255],
                _ => [0, 0, 0, 0],
            };
            let i = (y * w + x) * 4;
            out[i..i + 4].copy_from_slice(&px);
        }
    }
    (out, w as i32, h as i32)
}

/// Builds the built-in cursor buffer.
pub fn default_cursor() -> MemoryRenderBuffer {
    let (pixels, w, h) = arrow_pixels();
    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Argb8888,
        (w, h),
        1,
        Transform::Normal,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_has_an_opaque_tip_and_transparent_corner() {
        let (px, w, h) = arrow_pixels();
        assert_eq!((w, h), (12, 17));
        assert_eq!(&px[0..4], &[255, 255, 255, 255]);
        let last = ((w - 1) * 4) as usize;
        assert_eq!(px[last + 3], 0);
    }
}
