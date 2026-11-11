//! Real apps in a GPU session: the global list that `wayland-info` sees, vkcube at 144 Hz with
//! explicit sync, and a terminal window screenshot. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{
    CaptureOptions, CaptureTarget, Event, Fakeland, LaunchSpec, OutputConfig, WindowRef,
};
use fakeland_trace::{BufferKind, FrameTimeline, SyncKind};
use fakeland_types::{Hz, Nanos};

#[test]
#[ignore = "runs wayland-info in a session"]
fn wayland_info_sees_a_desktop_global_list() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-3")
                .mode(2560, 1440, Hz(143.912))
                .identity("Dell Inc.", "DELL S2721DGF", "4X1Y2Z3"),
        )
        .start()
        .expect("start");
    let app = session
        .blocking_launch(LaunchSpec::new("wayland-info"))
        .expect("launch");
    let exit = app.blocking_wait();
    assert!(exit.success(), "wayland-info failed: {}", app.stderr());
    let out = app.stdout();
    std::fs::write(support::out_dir().join("wayland-info.txt"), &out).expect("write");
    for global in [
        "wl_compositor",
        "wl_subcompositor",
        "xdg_wm_base",
        "zxdg_decoration_manager_v1",
        "zxdg_output_manager_v1",
        "wl_shm",
        "zwp_linux_dmabuf_v1",
        "wp_linux_drm_syncobj_manager_v1",
        "wp_presentation",
        "wp_fifo_manager_v1",
        "wp_commit_timing_manager_v1",
        "wp_viewporter",
        "wp_fractional_scale_manager_v1",
        "wp_single_pixel_buffer_manager_v1",
        "wp_alpha_modifier_v1",
        "wp_content_type_manager_v1",
        "wp_tearing_control_manager_v1",
        "wp_cursor_shape_manager_v1",
        "zwp_relative_pointer_manager_v1",
        "zwp_pointer_constraints_v1",
        "zwp_pointer_gestures_v1",
        "zwp_text_input_manager_v3",
        "zwp_input_method_manager_v2",
        "wl_data_device_manager",
        "zwp_primary_selection_device_manager_v1",
        "xdg_activation_v1",
        "zwp_idle_inhibit_manager_v1",
        "zwp_keyboard_shortcuts_inhibit_manager_v1",
        "zxdg_exporter_v2",
        "wl_seat",
        "wl_output",
    ] {
        assert!(out.contains(&format!("'{global}'")), "missing {global}");
    }
    assert!(out.contains("'wl_compositor',") && out.contains("version:  6"));
    assert!(out.contains("Dell Inc."), "output make");
    assert!(out.contains("DELL S2721DGF"), "output model");
    assert!(out.contains("DP-3"), "connector name");
    assert!(out.contains("refresh: 143.912 Hz"), "mode refresh");
    assert!(out.contains("seat0"));
    assert!(out.contains("main device"), "dmabuf feedback");
    assert!(
        !out.to_lowercase().contains("fakeland"),
        "nothing names fakeland"
    );
    let clients = session.blocking_clients().expect("clients");
    assert!(
        clients
            .iter()
            .all(|c| c.app.is_none() || c.app == Some(app.pid()))
    );
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs vkcube on the GPU"]
fn vkcube_presents_at_144_hz_with_explicit_sync() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1920, 1080, Hz(144.0)))
        .start()
        .expect("start");
    let app = session
        .blocking_launch(LaunchSpec::from_argv(["vkcube", "--wsi", "wayland"]).expect("argv"))
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no vkcube window ({e}): {}", app.stderr()));
    session
        .blocking_wait_frames(WindowRef::id(window.id), 20, Nanos::from_secs(5))
        .expect("frames");
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_secs(3));
    let trace = session.blocking_end_capture(capture).expect("trace");
    let shot = session
        .blocking_screenshot(
            CaptureTarget::Window {
                window: WindowRef::id(window.id),
            },
            CaptureOptions::default(),
        )
        .expect("screenshot");
    support::save(&shot.image, "vkcube.png");
    let validity = session.blocking_validity().expect("validity");
    session.blocking_shutdown().expect("shutdown");

    let timeline = FrameTimeline::build(&trace);
    let presented = timeline.presented_frames(window.surface, None);
    assert!(
        presented.len() > 300,
        "{} frames presented",
        presented.len()
    );
    let span = presented
        .last()
        .expect("frames")
        .time
        .since(presented[0].time)
        .as_secs_f64();
    let fps = (presented.len() - 1) as f64 / span;
    assert!(
        (140.0..=144.5).contains(&fps),
        "{fps:.1} fps over {span:.2} s"
    );
    let flags_ok = support::records(&trace, |e| matches!(e, Event::Presented { .. }))
        .all(|r| matches!(r.event, Event::Presented { flags, refresh, .. } if flags & 7 == 7 && refresh == Some(Nanos(6_944_444))));
    assert!(
        flags_ok,
        "presented flags carry vsync, hw clock and hw completion"
    );
    let content: Vec<_> = timeline
        .frames(window.surface)
        .iter()
        .filter(|f| f.buffer.is_some())
        .collect();
    assert!(
        content
            .iter()
            .all(|f| f.buffer.as_ref().unwrap().kind == BufferKind::Dmabuf)
    );
    assert!(
        content
            .iter()
            .filter(|f| f.sync == SyncKind::Explicit)
            .count()
            * 10
            >= content.len() * 9,
        "vkcube uses explicit sync on this driver"
    );
    assert!(
        content.iter().filter(|f| f.released.is_some()).count() + 4 >= content.len(),
        "buffers come back"
    );
    assert!(
        support::distinct_colors(&shot.image) > 8,
        "the screenshot shows the cube"
    );
    assert!(validity.wakes > 400);
    eprintln!(
        "vkcube {fps:.1} fps, max lateness {}",
        validity.max_lateness
    );
}

#[test]
#[ignore = "runs a GPU terminal"]
fn terminal_window_screenshot_shows_text() {
    let _serial = support::serial();
    let (program, args): (&str, Vec<&str>) = if support::have("alacritty") {
        (
            "alacritty",
            vec!["-e", "sh", "-c", "echo FAKELAND TERMINAL CHECK; sleep 30"],
        )
    } else {
        (
            "kitty",
            vec!["sh", "-c", "echo FAKELAND TERMINAL CHECK; sleep 30"],
        )
    };
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 800, Hz(60.0)))
        .start()
        .expect("start");
    let app = session
        .blocking_launch(LaunchSpec::new(program).args(args))
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_settled(
            WindowRef::id(window.id),
            Nanos::from_millis(300),
            Nanos::from_secs(10),
        )
        .expect("settled");
    let shot = session
        .blocking_screenshot(
            CaptureTarget::Window {
                window: WindowRef::id(window.id),
            },
            CaptureOptions {
                include_cursor: false,
                max_dim: Some(800),
            },
        )
        .expect("screenshot");
    support::save(&shot.image, "terminal.png");
    session.blocking_shutdown().expect("shutdown");
    assert_eq!(shot.image.width(), 800);
    assert!((shot.scale - 1.6).abs() < 1e-9, "logical px per image px");
    assert!(support::distinct_colors(&shot.image) > 3, "text is visible");
}
