//! Accuracy and timing reports of motion estimation on synthetic frames. Run them in release with
//! `--nocapture` to read the numbers; they assert accuracy and print timing.

use fakeland_types::{Clock, MonotonicClock, Rect};

use super::{Axis, MotionConfig, MotionEstimator};
use crate::image::Image;
use crate::synth::{add_noise, text_page, texture};

/// Runs a closure `runs` times and returns the mean milliseconds per run.
fn time_ms(runs: u32, mut f: impl FnMut()) -> f64 {
    let clock = MonotonicClock;
    let start = clock.now();
    for _ in 0..runs {
        f();
    }
    clock.now().since(start).as_millis_f64() / runs as f64
}

#[test]
fn subpixel_accuracy_report() {
    let mut e = MotionEstimator::new();
    let config = MotionConfig::with_max_shift(64);
    for (w, h, noise) in [
        (320, 240, 0),
        (320, 240, 10),
        (1280, 720, 0),
        (1280, 720, 10),
    ] {
        let base = texture(w, h, 0.0, 0.0, 99);
        let mut a = base.clone();
        if noise > 0 {
            add_noise(&mut a, noise, 7);
        }
        let (mut sum, mut max, mut n) = (0f64, 0f64, 0u32);
        for i in 0..12 {
            let dx = (i as f64 * 2.37) % 19.0 - 9.5;
            let dy = (i as f64 * 3.71) % 31.0 - 15.5;
            let mut b = texture(w, h, dx, dy, 99);
            if noise > 0 {
                add_noise(&mut b, noise, 8 + i);
            }
            let m = e
                .translation(&a, &b, None, &config)
                .unwrap_or_else(|r| panic!("({dx}, {dy}) at {w}x{h}: {r}"));
            let err = (m.dx - dx).abs().max((m.dy - dy).abs());
            sum += err;
            max = max.max(err);
            n += 1;
        }
        let mean = sum / n as f64;
        println!("translation {w}x{h} noise ±{noise}: mean error {mean:.3} px, max {max:.3} px");
        assert!(mean < 0.08 && max < 0.2, "mean {mean} max {max}");
    }
}

#[test]
fn full_hd_timing_report() {
    let a = text_page(1920, 1080, 0, 0, 5);
    let b = text_page(1920, 1080, 0, -37, 5);
    let mut e = MotionEstimator::new();
    let planar = MotionConfig::with_max_shift(200);
    let combined = MotionConfig {
        axis: Some(Axis::Vertical),
        ..planar
    };
    let m = e.translation(&a, &b, None, &planar).unwrap();
    assert!((m.dy + 37.0).abs() < 0.2 && m.dx.abs() < 0.2, "{m:?}");
    let runs = 10;
    let t_planar = time_ms(runs, || {
        let _ = e.translation(&a, &b, None, &planar);
    });
    let t_scroll = time_ms(runs, || {
        let _ = e.scroll_1d(&a, &b, None, Axis::Vertical, &planar);
    });
    let t_combined = time_ms(runs, || {
        let _ = e.estimate(&a, &b, None, &combined);
    });
    let roi = Some(Rect::new(200, 100, 1280, 800));
    let t_roi = time_ms(runs, || {
        let _ = e.translation(&a, &b, roi, &planar);
    });
    println!(
        "1920x1080: translation {t_planar:.2} ms, scroll_1d {t_scroll:.2} ms, combined \
         {t_combined:.2} ms, 1280x800 roi translation {t_roi:.2} ms"
    );
}

#[test]
fn full_hd_pixel_ops_timing_report() {
    let a = text_page(1920, 1080, 0, 0, 5);
    let mut b = a.clone();
    b.fill_rect(Rect::new(900, 500, 40, 20), [0, 0, 0, 255]);
    let runs = 10;
    let t_diff = time_ms(runs, || {
        let _ = crate::diff::diff(&a, &b, None, 0);
    });
    let t_changed = time_ms(runs, || {
        let _ = crate::diff::changed(&a, &b, None, 0);
    });
    let t_hash = time_ms(runs, || {
        let _ = crate::stability::region_hash(&a, None);
    });
    let damage = [Rect::new(890, 490, 60, 40)];
    let t_damage = time_ms(runs, || {
        let _ = crate::damage::damage_accuracy(&a, &b, &damage, 0);
    });
    let t_down = time_ms(runs, || {
        let _ = a.downscale_to_max_dim(1568);
    });
    let t_png = time_ms(2, || {
        let _ = a.encode_png();
    });
    let readback: Vec<u8> = a.as_bytes().to_vec();
    let t_readback = time_ms(runs, || {
        let _ = Image::from_readback(
            &readback,
            1920,
            1080,
            1920 * 4,
            crate::image::PixelFormat::Xrgb8888,
            crate::image::RowOrder::BottomUp,
        );
    });
    println!(
        "1920x1080: diff {t_diff:.2} ms, changed {t_changed:.2} ms, hash {t_hash:.2} ms, damage \
         {t_damage:.2} ms, downscale to 1568 {t_down:.2} ms, png {t_png:.2} ms, readback \
         {t_readback:.2} ms"
    );
}
