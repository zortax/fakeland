//! Rendering paths with the reference client: Pixman with shm buffers, pixel placement and
//! orientation, buffer sizes under fractional and integer scales, and large screenshots that
//! keep display deadlines. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{
    CaptureOptions, CaptureTarget, Event, Fakeland, LaunchSpec, OutputConfig, RendererChoice, Roi,
    WindowRef,
};
use fakeland_trace::BufferKind;
use fakeland_types::{Hz, Nanos, Rect};

/// The hover target of the client, in window-logical coordinates.
const HOVER: (i32, i32, i32, i32) = (100, 100, 200, 150);
const HOVER_COLOR: [u8; 3] = [0x30, 0x60, 0xc0];

fn hover_client() -> LaunchSpec {
    LaunchSpec::new(support::testclient()).args(["--scene", "hover", "--exit-after", "30"])
}

fn check_hover_pixels(renderer: RendererChoice, name: &str) {
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(800, 600, Hz(60.0)))
        .renderer(renderer)
        .start()
        .expect("start");
    let app = session.blocking_launch(hover_client()).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_frames(WindowRef::id(window.id), 2, Nanos::from_secs(5))
        .expect("frames");
    let shot = session
        .blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())
        .expect("screenshot");
    support::save(&shot.image, name);
    let region = session
        .blocking_screenshot(
            CaptureTarget::Roi {
                roi: Roi::window_rect(window.id, Rect::new(HOVER.0, HOVER.1, HOVER.2, HOVER.3)),
            },
            CaptureOptions::default(),
        )
        .expect("region");
    let trace = session.blocking_trace_snapshot().expect("trace");
    session.blocking_shutdown().expect("shutdown");

    let rgb = |x: u32, y: u32| {
        let p = shot.image.pixel(x, y).expect("pixel");
        [p[0], p[1], p[2]]
    };
    let (gx, gy) = (window.geometry.x as u32, window.geometry.y as u32);
    let inside = rgb(gx + HOVER.0 as u32 + 10, gy + HOVER.1 as u32 + 10);
    let above = rgb(gx + HOVER.0 as u32 + 10, gy + HOVER.1 as u32 - 10);
    let below = rgb(
        gx + HOVER.0 as u32 + 10,
        gy + (HOVER.1 + HOVER.3) as u32 + 10,
    );
    assert_eq!(
        inside, HOVER_COLOR,
        "the target sits at its logical position"
    );
    assert_ne!(above, HOVER_COLOR);
    assert_ne!(below, HOVER_COLOR);
    assert_eq!((region.image.width(), region.image.height()), (200, 150));
    let p = region.image.pixel(100, 75).expect("pixel");
    assert_eq!(
        [p[0], p[1], p[2]],
        HOVER_COLOR,
        "the region capture shows the target"
    );
    let shm = support::records(
        &trace,
        |e| matches!(e, Event::Commit { buffer: Some(b), .. } if b.kind == BufferKind::Shm),
    )
    .count();
    assert!(shm > 0, "the client uses shm buffers");
}

#[test]
#[ignore = "runs the reference client"]
fn pixman_renders_shm_clients_upright() {
    let _serial = support::serial();
    check_hover_pixels(RendererChoice::Pixman, "pixman-hover.png");
}

#[test]
#[ignore = "runs the reference client"]
fn gles_renders_shm_clients_upright() {
    let _serial = support::serial();
    check_hover_pixels(RendererChoice::Gles, "gles-hover.png");
}

fn buffer_width_at(scale: f64) -> (i32, i32, i32) {
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1920, 1080, Hz(60.0))
                .scale(scale),
        )
        .start()
        .expect("start");
    let app = session.blocking_launch(hover_client()).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_frames(WindowRef::id(window.id), 3, Nanos::from_secs(5))
        .expect("frames");
    let trace = session.blocking_trace_snapshot().expect("trace");
    session.blocking_shutdown().expect("shutdown");
    let last = support::records(&trace, |e| {
        matches!(e, Event::Commit { surface, buffer: Some(_), .. } if *surface == window.surface)
    })
    .last()
    .expect("a buffer commit");
    let Event::Commit {
        buffer: Some(b), ..
    } = &last.event
    else {
        unreachable!()
    };
    (b.width, b.height, window.geometry.w)
}

#[test]
#[ignore = "runs the reference client"]
fn fractional_scale_changes_buffer_sizes() {
    let _serial = support::serial();
    let (w15, h15, logical15) = buffer_width_at(1.5);
    assert_eq!(logical15, 1280);
    assert_eq!((w15, h15), (1920, 1080), "1.5 renders at full pixel size");
    let (w2, h2, logical2) = buffer_width_at(2.0);
    assert_eq!(logical2, 960);
    assert_eq!((w2, h2), (1920, 1080), "2.0 renders at full pixel size");
    let (w1, _, logical1) = buffer_width_at(1.0);
    assert_eq!((w1, logical1), (1920, 1920));
}

#[test]
#[ignore = "runs the reference client"]
fn large_screenshots_keep_display_deadlines() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(3840, 2160, Hz(60.0)))
        .renderer(RendererChoice::Gles)
        .start()
        .expect("start");
    let app = session.blocking_launch(hover_client()).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_frames(WindowRef::id(window.id), 3, Nanos::from_secs(5))
        .expect("frames");
    // A blocking capture at this size takes longer than a refresh. The client draws into shm
    // buffers, and each capture uploads a new one in one GL call; the first upload also allocates
    // the texture, so it runs before the measured part.
    session
        .blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())
        .expect("first screenshot");
    fakeland::block_on(session.reset_validity()).expect("reset");
    let shots = 8;
    for _ in 0..shots {
        let shot = session
            .blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())
            .expect("screenshot");
        assert_eq!((shot.image.width(), shot.image.height()), (3840, 2160));
    }
    let validity = session.blocking_validity().expect("validity");
    let trace = session.blocking_trace_snapshot().expect("trace");
    session.blocking_shutdown().expect("shutdown");
    let costs: Vec<i64> = support::records(&trace, |e| matches!(e, Event::Capture { .. }))
        .skip(1)
        .filter_map(|r| match &r.event {
            Event::Capture { cost, .. } => Some(cost.as_nanos()),
            _ => None,
        })
        .collect();
    support::summarize("4K screenshot cost (sum of steps)", &costs);
    eprintln!(
        "wakes {}, max lateness {:.3} ms",
        validity.wakes,
        validity.max_lateness.as_millis_f64()
    );
    assert_eq!(costs.len(), shots);
    assert_eq!(
        validity.late_wakes,
        0,
        "max lateness {:.3} ms",
        validity.max_lateness.as_millis_f64()
    );
}
