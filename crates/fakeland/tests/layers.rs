//! Layer-shell surfaces with the layer client: work areas from exclusive zones, stacking in
//! screenshots, clicks by coordinates, input regions, keyboard focus rules, popups and close on
//! output removal. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{
    Fakeland, KeyboardMode, OutputConfig, ShellLayer, WindowKind, WindowOp, WindowOpError,
    WindowRef,
};
use fakeland_types::{Hz, Nanos, Size};
use support::shell::{
    click, count_events, is, layer, pixel_at, session, toplevel, wait_event, wait_info, wait_layer,
    wait_pixel,
};

#[test]
#[ignore = "runs the layer client"]
fn bar_reserves_its_zone_and_takes_clicks() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.layer");
    let bar = s
        .blocking_launch(layer(&[
            "--namespace",
            "bar",
            "--anchor",
            "top,left,right",
            "--size",
            "0x40",
            "--exclusive-zone",
            "40",
            "--color",
            "#ff0000",
        ]))
        .expect("launch bar");
    let info = wait_layer(&s, &bar, "bar");
    assert_eq!(info.kind, WindowKind::Layer);
    let l = info.layer.clone().expect("layer info");
    assert_eq!(
        (l.layer, l.keyboard, l.exclusive_zone),
        (ShellLayer::Top, KeyboardMode::None, 40)
    );
    assert_eq!(l.anchor, vec!["top", "left", "right"]);
    assert_eq!(
        (
            info.geometry.x,
            info.geometry.y,
            info.geometry.w,
            info.geometry.h
        ),
        (0, 0, 1280, 40)
    );
    let moved = wait_info(&s, WindowRef::id(window.id), |w| w.geometry.y == 40);
    assert_eq!((moved.geometry.w, moved.geometry.h), (1280, 680));
    assert!(
        moved.focused,
        "a bar without keyboard interactivity leaves focus alone"
    );

    assert_eq!(pixel_at(&s, 640, 20), [255, 0, 0]);
    assert_ne!(pixel_at(&s, 640, 400), [255, 0, 0]);
    click(&s, 100.0, 20.0);
    let press = wait_event(&bar, "button", |e| is(e, "button") && e["pressed"] == true);
    assert_eq!(press["surface"], "layer");
    assert_eq!(
        (press["x"].as_f64(), press["y"].as_f64()),
        (Some(100.0), Some(20.0))
    );
    let after = s
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert!(after.focused);
    let refused = s.blocking_window_op(WindowRef::layer("bar"), WindowOp::Focus);
    assert!(
        matches!(
            refused,
            Err(fakeland::SessionError::WindowOp(
                WindowOpError::NoKeyboardFocus(id)
            )) if id == info.id
        ),
        "a bar without keyboard interactivity refuses focus: {refused:?}"
    );

    s.blocking_window_op(WindowRef::layer("bar"), WindowOp::Close)
        .expect("close");
    wait_event(&bar, "closed", |e| is(e, "closed"));
    let back = wait_info(&s, WindowRef::id(window.id), |w| w.geometry.y == 0);
    assert_eq!(back.geometry.h, 720);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn on_demand_layers_take_focus_and_honor_input_regions() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.focus");
    let menu = s
        .blocking_launch(layer(&[
            "--namespace",
            "menu",
            "--layer",
            "overlay",
            "--keyboard",
            "on-demand",
            "--anchor",
            "top,left",
            "--size",
            "300x200",
            "--margin",
            "100,0,0,100",
            "--input-region",
            "0,0,300,100",
            "--popup",
            "20,150,120,80",
            "--color",
            "#00ff00",
            "--popup-color",
            "#0000ff",
        ]))
        .expect("launch menu");
    let info = wait_layer(&s, &menu, "menu");
    assert_eq!(
        (
            info.geometry.x,
            info.geometry.y,
            info.geometry.w,
            info.geometry.h
        ),
        (100, 100, 300, 200)
    );
    wait_event(&menu, "keyboard enter on map", |e| is(e, "keyboard_enter"));
    let focused = wait_info(&s, WindowRef::layer("menu"), |w| w.focused);
    assert!(focused.focused);
    s.blocking_key("a").expect("key");
    wait_event(&menu, "key", |e| is(e, "key") && e["key"] == 30);

    // The popup sits at 120,250 and shows above the toplevel.
    wait_event(&menu, "popup configure", |e| is(e, "popup_configure"));
    s.blocking_wait_settled(
        WindowRef::layer("menu"),
        Nanos::from_millis(100),
        Nanos::from_secs(5),
    )
    .expect("settled");
    assert_eq!(pixel_at(&s, 200, 150), [0, 255, 0]);
    assert_eq!(pixel_at(&s, 150, 300), [0, 0, 255]);

    // Below the input region the press reaches the toplevel, which takes focus.
    click(&s, 350.0, 250.0);
    wait_event(&menu, "keyboard leave", |e| is(e, "keyboard_leave"));
    assert_eq!(count_events(&menu, |e| is(e, "button")), 0);
    let w = wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    assert!(w.focused);

    // A press in the input region gives the layer surface focus again.
    click(&s, 200.0, 150.0);
    let press = wait_event(&menu, "button", |e| is(e, "button"));
    assert_eq!(press["surface"], "layer");
    wait_info(&s, WindowRef::layer("menu"), |w| w.focused);
    assert_eq!(count_events(&menu, |e| is(e, "keyboard_enter")), 2);

    // The popup takes input.
    click(&s, 150.0, 300.0);
    wait_event(&menu, "popup button", |e| {
        is(e, "button") && e["surface"] == "popup"
    });
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn exclusive_layers_keep_focus_and_layers_stack_around_toplevels() {
    let _serial = support::serial();
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .floating(Size::new(400, 300))
        .start()
        .expect("start");
    let (_top, window) = toplevel(&s, "org.example.stack");
    let wallpaper = s
        .blocking_launch(layer(&[
            "--namespace",
            "wallpaper",
            "--layer",
            "background",
            "--anchor",
            "top,bottom,left,right",
            "--size",
            "0x0",
            "--exclusive-zone",
            "-1",
            "--color",
            "#808000",
        ]))
        .expect("launch wallpaper");
    wait_layer(&s, &wallpaper, "wallpaper");
    s.blocking_wait_settled(
        WindowRef::layer("wallpaper"),
        Nanos::from_millis(100),
        Nanos::from_secs(5),
    )
    .expect("settled");
    let g = window.geometry;
    assert_eq!(pixel_at(&s, 10, 10), [128, 128, 0]);
    assert_ne!(
        pixel_at(&s, (g.x + g.w / 2) as u32, (g.y + g.h / 2) as u32),
        [128, 128, 0],
        "the toplevel covers the background layer"
    );

    let dialog = s
        .blocking_launch(layer(&[
            "--namespace",
            "dialog",
            "--layer",
            "overlay",
            "--keyboard",
            "exclusive",
            "--size",
            "200x100",
        ]))
        .expect("launch dialog");
    wait_event(&dialog, "keyboard enter", |e| is(e, "keyboard_enter"));
    click(&s, 10.0, 10.0);
    click(&s, (g.x + 10) as f64, (g.y + 10) as f64);
    s.blocking_key("b").expect("key");
    wait_event(&dialog, "key", |e| is(e, "key") && e["key"] == 48);
    assert_eq!(count_events(&dialog, |e| is(e, "keyboard_leave")), 0);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn the_latest_exclusive_overlay_takes_focus_and_stacks_on_top() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let exclusive = |namespace: &str, color: &str| {
        layer(&[
            "--namespace",
            namespace,
            "--layer",
            "overlay",
            "--keyboard",
            "exclusive",
            "--size",
            "200x100",
            "--color",
            color,
        ])
    };
    let first = s
        .blocking_launch(exclusive("first", "#ff0000"))
        .expect("launch first");
    wait_event(&first, "keyboard enter", |e| is(e, "keyboard_enter"));
    let second = s
        .blocking_launch(exclusive("second", "#00ff00"))
        .expect("launch second");
    wait_layer(&s, &second, "second");
    wait_event(&second, "keyboard enter", |e| is(e, "keyboard_enter"));
    wait_event(&first, "keyboard leave", |e| is(e, "keyboard_leave"));
    wait_info(&s, WindowRef::layer("second"), |w| w.focused);
    wait_pixel(&s, 640, 360, [0, 255, 0]);

    // A press on the shared spot reaches the top surface, and keys follow it.
    click(&s, 640.0, 360.0);
    let press = wait_event(&second, "button", |e| is(e, "button"));
    assert_eq!(press["surface"], "layer");
    assert_eq!(count_events(&first, |e| is(e, "button")), 0);
    s.blocking_key("c").expect("key");
    wait_event(&second, "key", |e| is(e, "key") && e["key"] == 46);
    assert_eq!(count_events(&first, |e| is(e, "key")), 0);

    // When the top surface goes, the one below takes focus back.
    s.blocking_window_op(WindowRef::layer("second"), WindowOp::Close)
        .expect("close");
    wait_event(&second, "closed", |e| is(e, "closed"));
    poll_focus(&first);
    s.blocking_shutdown().expect("shutdown");
}

/// Waits until a client got keyboard focus a second time.
fn poll_focus(app: &fakeland::App) {
    support::shell::poll(
        || format!("no second keyboard enter: {}", app.stdout()),
        || (count_events(app, |e| is(e, "keyboard_enter")) >= 2).then_some(()),
    );
}

#[test]
#[ignore = "runs the layer client"]
fn removed_outputs_close_their_layers() {
    let _serial = support::serial();
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .output(OutputConfig::new("DP-2").mode(1024, 768, Hz(60.0)))
        .start()
        .expect("start");
    let outputs = s.blocking_outputs().expect("outputs");
    let panel = s
        .blocking_launch(layer(&[
            "--namespace",
            "panel",
            "--output",
            "1",
            "--anchor",
            "bottom,left,right",
            "--size",
            "0x30",
        ]))
        .expect("launch panel");
    let info = wait_layer(&s, &panel, "panel");
    assert_eq!(info.output, outputs[1].id);
    assert_eq!(
        (info.geometry.x, info.geometry.y, info.geometry.w),
        (1280, 738, 1024)
    );
    s.blocking_remove_output(outputs[1].id).expect("remove");
    wait_event(&panel, "closed", |e| is(e, "closed"));
    let windows = s.blocking_windows().expect("windows");
    assert!(windows.iter().all(|w| w.namespace() != Some("panel")));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn presses_outside_input_regions_fall_through() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let desk = s
        .blocking_launch(layer(&[
            "--namespace",
            "desk",
            "--layer",
            "bottom",
            "--anchor",
            "top,bottom,left,right",
            "--size",
            "0x0",
        ]))
        .expect("launch desk");
    wait_layer(&s, &desk, "desk");
    let app = s
        .blocking_launch(layer(&[
            "--mode",
            "toplevel",
            "--namespace",
            "org.example.region",
            "--input-region",
            "0,0,200,200",
            "--popup",
            "400,10,100,100",
        ]))
        .expect("launch toplevel");
    let window = s
        .blocking_wait_window(
            WindowRef::app_id("org.example.region"),
            Nanos::from_secs(10),
        )
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    assert_eq!(window.kind, WindowKind::Toplevel);
    wait_event(&app, "popup configure", |e| is(e, "popup_configure"));
    s.blocking_wait_settled(
        WindowRef::id(window.id),
        Nanos::from_millis(100),
        Nanos::from_secs(5),
    )
    .expect("settled");

    click(&s, 100.0, 100.0);
    let press = wait_event(&app, "toplevel button", |e| is(e, "button"));
    assert_eq!(press["surface"], "toplevel");
    click(&s, 450.0, 50.0);
    wait_event(&app, "popup button", |e| {
        is(e, "button") && e["surface"] == "popup"
    });
    click(&s, 800.0, 500.0);
    let press = wait_event(&desk, "desk button", |e| is(e, "button"));
    assert_eq!(press["surface"], "layer");
    assert_eq!(
        (press["x"].as_f64(), press["y"].as_f64()),
        (Some(800.0), Some(500.0))
    );
    s.blocking_shutdown().expect("shutdown");
}
