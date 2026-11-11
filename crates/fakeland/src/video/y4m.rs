//! YUV4MPEG2 output: the stream header and the conversion of RGBA frames to 4:2:0 planes with
//! BT.601 limited-range coefficients.

use fakeland_vision::ImageView;

use super::FrameRate;

/// Returns the stream header of a 4:2:0 progressive stream.
pub fn header(width: u32, height: u32, rate: FrameRate) -> String {
    format!(
        "YUV4MPEG2 W{width} H{height} F{}:{} Ip A1:1 C420jpeg\n",
        rate.num, rate.den
    )
}

/// The frame marker before each frame.
pub const FRAME: &[u8] = b"FRAME\n";

fn luma(r: i32, g: i32, b: i32) -> u8 {
    (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16).clamp(0, 255) as u8
}

fn chroma(r: i32, g: i32, b: i32) -> (u8, u8) {
    let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
    let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
    (u.clamp(0, 255) as u8, v.clamp(0, 255) as u8)
}

/// Converts a view with even dimensions into Y, U and V planes, in that order, into `out`.
pub fn to_i420(view: &ImageView<'_>, out: &mut Vec<u8>) {
    let (w, h) = (view.width() as usize, view.height() as usize);
    out.clear();
    out.resize(w * h + 2 * (w / 2) * (h / 2), 0);
    let (y_plane, rest) = out.split_at_mut(w * h);
    let (u_plane, v_plane) = rest.split_at_mut((w / 2) * (h / 2));
    for y in 0..h {
        let row = view.row(y as u32);
        for x in 0..w {
            let p = &row[x * 4..x * 4 + 4];
            y_plane[y * w + x] = luma(p[0].into(), p[1].into(), p[2].into());
        }
    }
    for cy in 0..h / 2 {
        let (r0, r1) = (view.row(cy as u32 * 2), view.row(cy as u32 * 2 + 1));
        for cx in 0..w / 2 {
            let mut sum = [0i32; 3];
            for row in [r0, r1] {
                for dx in 0..2 {
                    let i = (cx * 2 + dx) * 4;
                    for c in 0..3 {
                        sum[c] += i32::from(row[i + c]);
                    }
                }
            }
            let (u, v) = chroma((sum[0] + 2) / 4, (sum[1] + 2) / 4, (sum[2] + 2) / 4);
            u_plane[cy * (w / 2) + cx] = u;
            v_plane[cy * (w / 2) + cx] = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_vision::Image;

    use super::*;

    #[test]
    fn header_names_size_and_rate() {
        let r = FrameRate::new(60000, 1001).unwrap();
        assert_eq!(
            header(640, 360, r),
            "YUV4MPEG2 W640 H360 F60000:1001 Ip A1:1 C420jpeg\n"
        );
    }

    #[test]
    fn converts_black_white_and_red() {
        let mut out = Vec::new();
        to_i420(&Image::filled(4, 2, [0, 0, 0, 255]).view(), &mut out);
        assert_eq!(out.len(), 8 + 2 + 2);
        assert!(out[..8].iter().all(|&y| y == 16));
        assert!(out[8..].iter().all(|&c| c == 128));
        to_i420(&Image::filled(2, 2, [255, 255, 255, 255]).view(), &mut out);
        assert_eq!(out, vec![235, 235, 235, 235, 128, 128]);
        to_i420(&Image::filled(2, 2, [255, 0, 0, 255]).view(), &mut out);
        assert_eq!(&out[..4], &[82, 82, 82, 82]);
        assert_eq!(&out[4..], &[90, 240]);
    }
}
