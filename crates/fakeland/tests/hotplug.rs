//! Output hotplug with the layer client: an unplugged output loses its `wl_output` global and
//! comes back with its config, and the session lock covers plugged outputs. Run with
//! `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{App, Fakeland, OutputConfig, Session, WindowKind, WindowRef};
use fakeland_types::{Hz, OutputId};
use serde_json::Value;
use support::shell::{
    click, count_events, is, layer, poll, session, wait_count, wait_event, wait_info, wait_layer,
    wait_pixel, wait_pixel_on,
};

/// Launches a layer surface on an output index of the client registry.
fn panel(s: &Session, namespace: &str, output: &str) -> App {
    let app = s
        .blocking_launch(layer(&[
            "--namespace",
            namespace,
            "--output",
            output,
            "--anchor",
            "top,left,right",
            "--size",
            "0x30",
        ]))
        .expect("launch panel");
    wait_layer(s, &app, namespace);
    app
}

/// Locks the session with the layer client and waits for the lock.
fn lock(s: &Session, args: &[&str]) -> App {
    let mut all = vec!["--mode", "lock"];
    all.extend_from_slice(args);
    let lock = s.blocking_launch(layer(&all)).expect("launch lock");
    wait_event(&lock, "locked", |e| is(e, "locked") || is(e, "finished"));
    assert_eq!(count_events(&lock, |e| is(e, "finished")), 0, "refused");
    lock
}

/// Returns the count of lock surfaces in the window list.
fn lock_surfaces(s: &Session) -> usize {
    let windows = s.blocking_windows().expect("windows");
    windows
        .iter()
        .filter(|w| w.kind == WindowKind::Lock)
        .count()
}

/// Waits until the window list holds a count of lock surfaces.
fn wait_lock_surfaces(s: &Session, n: usize) {
    poll(
        || format!("{} lock surfaces, expected {n}", lock_surfaces(s)),
        || (lock_surfaces(s) == n).then_some(()),
    );
}

/// Returns the id of the output with a connector name.
fn output_named(s: &Session, name: &str) -> OutputId {
    let outputs = s.blocking_outputs().expect("outputs");
    outputs
        .iter()
        .find(|o| o.name == name)
        .unwrap_or_else(|| panic!("no output {name}: {outputs:?}"))
        .id
}

#[test]
#[ignore = "runs the layer client"]
fn an_unplugged_output_comes_back_with_its_global_and_config() {
    let _serial = support::serial();
    let s = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .output(
            OutputConfig::new("DP-2")
                .mode(2048, 1536, Hz(60.0))
                .scale(2.0)
                .identity("Acme", "Wide 27", "A-42"),
        )
        .start()
        .expect("start");
    let watcher = panel(&s, "watcher", "0");
    let side = panel(&s, "side", "1");
    wait_count(&watcher, "output_name", 2);
    let global = |name: &str| -> Value {
        wait_event(&watcher, name, |e| {
            is(e, "output_name") && e["name"] == name
        })["global"]
            .clone()
    };
    let old = global("DP-2");

    let dp2 = output_named(&s, "DP-2");
    s.blocking_remove_output(dp2).expect("unplug");
    let removed = wait_event(&watcher, "output_removed", |e| is(e, "output_removed"));
    assert_eq!(removed["global"], old, "the DP-2 global goes away");
    wait_event(&side, "closed", |e| is(e, "closed"));
    assert_eq!(s.blocking_outputs().expect("outputs").len(), 1);
    let unplugged = s.blocking_unplugged_outputs().expect("unplugged");
    assert_eq!(unplugged.len(), 1);
    assert_eq!(unplugged[0].name, "DP-2");

    let back = s.blocking_replug_output(None).expect("replug");
    assert_ne!(back, dp2, "the output gets a new id");
    let added = wait_event(&watcher, "output_added", |e| is(e, "output_added"));
    assert_ne!(added["global"], old, "the client sees a new global");
    wait_count(&watcher, "output_name", 3);
    let outputs = s.blocking_outputs().expect("outputs");
    let o = outputs.iter().find(|o| o.id == back).expect("replugged");
    assert_eq!((o.name.as_str(), o.make.as_str()), ("DP-2", "Acme"));
    assert_eq!((o.mode.width, o.mode.height, o.scale), (2048, 1536, 2.0));
    assert_eq!((o.position.x, o.position.y), (1280, 0));
    assert!(
        s.blocking_unplugged_outputs()
            .expect("unplugged")
            .is_empty()
    );
    assert!(
        s.blocking_replug_output(None).is_err(),
        "nothing is unplugged"
    );
    assert!(s.blocking_replug_output(Some("HDMI-A-9".into())).is_err());

    // A new layer surface on the replugged output maps there.
    let again = panel(&s, "side-again", "1");
    let info = wait_layer(&s, &again, "side-again");
    assert_eq!(info.output, back);
    assert_eq!((info.geometry.x, info.geometry.w), (1280, 1024));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn the_lock_covers_a_plugged_output_and_survives_its_unplug() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let lock = lock(&s, &["--unlock-on-input", "--color", "#4040ff"]);
    wait_lock_surfaces(&s, 1);
    let dp2 = s
        .blocking_add_output(OutputConfig::new("DP-2").mode(1024, 768, Hz(60.0)))
        .expect("plug");
    wait_event(&lock, "output_added", |e| is(e, "output_added"));
    wait_lock_surfaces(&s, 2);
    wait_pixel_on(&s, Some(dp2), 500, 400, [64, 64, 255]);

    s.blocking_remove_output(dp2).expect("unplug");
    wait_lock_surfaces(&s, 1);
    wait_pixel(&s, 640, 400, [64, 64, 255]);
    let back = s.blocking_replug_output(None).expect("replug");
    wait_lock_surfaces(&s, 2);
    wait_pixel_on(&s, Some(back), 500, 400, [64, 64, 255]);

    // The press on the replugged output reaches the lock surface there and unlocks.
    click(&s, 1280.0 + 500.0, 400.0);
    let press = wait_event(&lock, "lock button", |e| is(e, "button"));
    assert_eq!(press["surface"], "lock2");
    wait_event(&lock, "unlocked", |e| is(e, "unlocked"));
    wait_lock_surfaces(&s, 0);
    assert!(!s.blocking_force_unlock().expect("force unlock"));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn lock_surfaces_of_a_force_unlocked_client_stay_hidden() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let stale = lock(&s, &["--color", "#4040ff"]);
    assert!(s.blocking_force_unlock().expect("force unlock"));
    let next = lock(&s, &["--unlock-on-input", "--color", "#40ff40"]);
    wait_lock_surfaces(&s, 1);

    // Both clients create a lock surface on the new output. Only the holder's shows.
    let dp2 = s
        .blocking_add_output(OutputConfig::new("DP-2").mode(1024, 768, Hz(60.0)))
        .expect("plug");
    wait_count(&stale, "lock_configure", 2);
    wait_count(&next, "lock_configure", 2);
    wait_pixel_on(&s, Some(dp2), 500, 400, [64, 255, 64]);
    assert_eq!(lock_surfaces(&s), 2);
    let surface = wait_info(&s, WindowRef::Lock, |w| w.focused);
    assert_eq!(surface.app, Some(next.pid()));
    for app in [&stale, &next] {
        assert!(
            !app.stderr().contains("error"),
            "a lock client hit an error: {}",
            app.stderr()
        );
    }

    click(&s, 1280.0 + 500.0, 400.0);
    assert_eq!(count_events(&stale, |e| is(e, "button")), 0);
    wait_event(&next, "unlocked", |e| is(e, "unlocked"));
    assert!(!s.blocking_force_unlock().expect("force unlock"));
    s.blocking_shutdown().expect("shutdown");
}
