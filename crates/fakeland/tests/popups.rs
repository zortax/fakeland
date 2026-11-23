//! Popup grabs with the layer client and a GTK popover: the grabbed popup takes keyboard focus,
//! presses inside the popup chain or on its own client keep it, and presses elsewhere dismiss it
//! with `popup_done` and reach the surface under the pointer. Run with
//! `cargo test -p fakeland -- --ignored`.

mod support;

use std::path::PathBuf;

use fakeland::{App, Fakeland, LaunchSpec, OutputConfig, Session, WindowRef};
use fakeland_types::{Hz, Nanos, Size};
use support::shell::{
    click, count_events, is, layer, pixel_at, poll, session, toplevel, wait_count, wait_event,
    wait_info, wait_layer, wait_pixel,
};

/// Launches a red bar along the top edge whose popup grabs the seat.
fn grabbing_bar(s: &Session) -> App {
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
            "--popup",
            "20,40,200,100",
            "--popup-color",
            "#0000ff",
            "--popup-grab",
        ]))
        .expect("launch bar");
    wait_layer(s, &bar, "bar");
    bar
}

/// Returns the count of button presses of a client.
fn presses(app: &App) -> usize {
    count_events(app, |e| is(e, "button") && e["pressed"] == true)
}

#[test]
#[ignore = "runs the layer client"]
fn a_layer_popup_grab_holds_the_keyboard_until_a_press_elsewhere() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.grab");
    let bar = grabbing_bar(&s);
    wait_info(&s, WindowRef::id(window.id), |w| w.geometry.y == 40);

    // A press on the bar opens the popup at 20,40 with a grab.
    click(&s, 100.0, 20.0);
    wait_event(&bar, "popup configure", |e| is(e, "popup_configure"));
    let enter = wait_event(&bar, "popup keyboard enter", |e| {
        is(e, "keyboard_enter") && e["surface"] == "popup"
    });
    assert_eq!(enter["surface"], "popup");
    wait_pixel(&s, 100, 80, [0, 0, 255]);
    s.blocking_key("a").expect("key");
    wait_event(&bar, "key", |e| is(e, "key") && e["key"] == 30);

    // Presses in the popup and on the bar keep the grab.
    click(&s, 100.0, 80.0);
    wait_event(&bar, "popup button", |e| {
        is(e, "button") && e["surface"] == "popup"
    });
    click(&s, 600.0, 20.0);
    poll(
        || format!("no third press: {}", bar.stdout()),
        || (presses(&bar) == 3).then_some(()),
    );
    assert_eq!(count_events(&bar, |e| is(e, "popup_done")), 0);

    // A press on the toplevel of another client dismisses the popup and reaches the toplevel.
    click(&s, 640.0, 400.0);
    wait_event(&bar, "popup done", |e| is(e, "popup_done"));
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    poll(
        || "the popup stays on screen".to_owned(),
        || (pixel_at(&s, 100, 80) != [0, 0, 255]).then_some(()),
    );
    assert_eq!(
        presses(&bar),
        3,
        "the dismissing press goes to the toplevel"
    );

    // The next press on the bar opens the popup again.
    click(&s, 100.0, 20.0);
    wait_count(&bar, "popup_configure", 2);
    wait_count(&bar, "keyboard_enter", 2);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_toplevel_popup_grab_ends_at_a_press_on_no_surface() {
    let _serial = support::serial();
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .floating(Size::new(400, 300))
        .start()
        .expect("start");
    let app = s
        .blocking_launch(layer(&[
            "--mode",
            "toplevel",
            "--namespace",
            "org.example.menu",
            "--popup",
            "50,50,150,100",
            "--popup-grab",
        ]))
        .expect("launch toplevel");
    let window = s
        .blocking_wait_window(WindowRef::app_id("org.example.menu"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let g = window.geometry;
    wait_event(&app, "keyboard enter", |e| {
        is(e, "keyboard_enter") && e["surface"] == "toplevel"
    });

    click(&s, (g.x + 10) as f64, (g.y + 10) as f64);
    wait_event(&app, "popup keyboard enter", |e| {
        is(e, "keyboard_enter") && e["surface"] == "popup"
    });
    click(&s, (g.x + 100) as f64, (g.y + 100) as f64);
    wait_event(&app, "popup button", |e| {
        is(e, "button") && e["surface"] == "popup"
    });
    assert_eq!(count_events(&app, |e| is(e, "popup_done")), 0);

    // The space around the floating window holds no surface.
    click(&s, 5.0, 5.0);
    wait_event(&app, "popup done", |e| is(e, "popup_done"));
    wait_count(&app, "keyboard_enter", 3);
    let enters: Vec<_> = support::shell::events(&app)
        .into_iter()
        .filter(|e| is(e, "keyboard_enter"))
        .collect();
    assert_eq!(enters[2]["surface"], "toplevel");
    s.blocking_shutdown().expect("shutdown");
}

fn popover_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtk_popover.py")
}

fn have_gtk4() -> bool {
    support::have("python3")
        && std::process::Command::new("python3")
            .args([
                "-c",
                "import gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk",
            ])
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .output()
            .is_ok_and(|o| o.status.success())
}

/// Waits until the GTK script printed a word a number of times.
fn wait_word(app: &App, word: &str, n: usize) {
    poll(
        || {
            format!(
                "fewer than {n} {word:?}: {}\n{}",
                app.stdout(),
                app.stderr()
            )
        },
        || (app.stdout().lines().filter(|l| *l == word).count() >= n).then_some(()),
    );
}

#[test]
#[ignore = "runs python3 with GTK 4"]
fn gtk_popovers_take_keys_and_close_on_presses_elsewhere() {
    if !have_gtk4() {
        eprintln!("python3 with GTK 4 is missing");
        return;
    }
    let _serial = support::serial();
    let s = session(1280, 720);
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
        ]))
        .expect("launch bar");
    wait_layer(&s, &bar, "bar");
    let gtk = s
        .blocking_launch(
            LaunchSpec::new("python3").args([popover_script().to_string_lossy().into_owned()]),
        )
        .expect("launch gtk");
    let window = s
        .blocking_wait_window(
            WindowRef::app_id("org.example.popover"),
            Nanos::from_secs(20),
        )
        .unwrap_or_else(|e| panic!("no window ({e}): {}", gtk.stderr()));
    wait_word(&gtk, "ready", 1);
    s.blocking_wait_settled(
        WindowRef::id(window.id),
        Nanos::from_millis(200),
        Nanos::from_secs(10),
    )
    .expect("settled");
    let g = window.geometry;
    let (cx, cy) = ((g.x + g.w / 2) as f64, (g.y + g.h / 2) as f64);

    // The open popover takes keys, and Escape closes it.
    click(&s, cx, cy);
    wait_word(&gtk, "shown", 1);
    s.blocking_key("a").expect("key");
    wait_word(&gtk, "key", 1);
    s.blocking_key("Escape").expect("escape");
    wait_word(&gtk, "closed", 1);

    // A press on the bar of another client closes it, and the bar gets the press.
    click(&s, cx, cy);
    wait_word(&gtk, "shown", 2);
    click(&s, 640.0, 20.0);
    wait_word(&gtk, "closed", 2);
    let press = wait_event(&bar, "bar button", |e| is(e, "button"));
    assert_eq!(press["surface"], "layer");
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    s.blocking_shutdown().expect("shutdown");
}
