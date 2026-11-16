//! The session lock with the layer client: lock surfaces that hide and take everything, unlock,
//! relock, a lock whose client died, the forced unlock, a refused second lock, and unlock requests
//! of stale lock objects. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{App, Session, WindowKind, WindowRef};
use fakeland_types::Nanos;
use support::shell::{
    click, count_events, is, layer, pixel_at, session, toplevel, wait_count, wait_event, wait_info,
    wait_layer, wait_pixel,
};

/// Launches a red bar along the top edge.
fn bar(s: &Session) -> App {
    let bar = s
        .blocking_launch(layer(&[
            "--namespace",
            "bar",
            "--anchor",
            "top,left,right",
            "--size",
            "0x40",
            "--color",
            "#ff0000",
        ]))
        .expect("launch bar");
    wait_layer(s, &bar, "bar");
    bar
}

/// Locks the session with the layer client and waits for the lock.
fn lock(s: &Session, args: &[&str]) -> App {
    let mut all = vec!["--mode", "lock"];
    all.extend_from_slice(args);
    let lock = s.blocking_launch(layer(&all)).expect("launch lock");
    wait_event(&lock, "locked", |e| is(e, "locked") || is(e, "finished"));
    assert_eq!(
        count_events(&lock, |e| is(e, "finished")),
        0,
        "the lock was refused: {}",
        lock.stdout()
    );
    lock
}

#[test]
#[ignore = "runs the layer client"]
fn the_lock_hides_everything_until_unlock() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.locked");
    let bar = bar(&s);
    let lock = lock(&s, &["--unlock-on-input", "--color", "#4040ff"]);
    let surface = s
        .blocking_wait_window(WindowRef::Lock, Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no lock surface ({e}): {}", lock.stderr()));
    assert_eq!(surface.kind, WindowKind::Lock);
    assert_eq!((surface.geometry.w, surface.geometry.h), (1280, 720));
    wait_info(&s, WindowRef::Lock, |w| w.focused);
    s.blocking_wait_settled(
        WindowRef::Lock,
        Nanos::from_millis(100),
        Nanos::from_secs(5),
    )
    .expect("settled");
    assert_eq!(pixel_at(&s, 640, 20), [64, 64, 255], "the bar stays hidden");
    assert_eq!(pixel_at(&s, 640, 400), [64, 64, 255]);

    // A click on the bar position reaches the lock surface, which unlocks.
    click(&s, 640.0, 20.0);
    assert_eq!(count_events(&bar, |e| is(e, "button")), 0);
    let press = wait_event(&lock, "lock button", |e| is(e, "button"));
    assert_eq!(press["surface"], "lock0");
    wait_event(&lock, "unlocked", |e| is(e, "unlocked"));
    let back = wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    assert!(back.focused);
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_dead_lock_client_keeps_the_lock_until_the_next_lock() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.relock");
    let _bar = bar(&s);
    let first = lock(&s, &["--color", "#4040ff"]);
    wait_info(&s, WindowRef::Lock, |w| w.focused);
    wait_pixel(&s, 640, 20, [64, 64, 255]);

    // The client dies without unlocking: the session stays locked behind a black backdrop.
    first.kill().expect("kill");
    first.blocking_wait();
    poll_no_lock_surface(&s);
    wait_pixel(&s, 640, 20, [0, 0, 0]);
    click(&s, 640.0, 400.0);
    let w = s
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert!(!w.focused, "the toplevel stays out of reach");

    // The next lock takes over, and its unlock ends the lock.
    let second = lock(&s, &["--unlock-on-input", "--color", "#40ff40"]);
    let surface = s
        .blocking_wait_window(WindowRef::Lock, Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no lock surface ({e}): {}", second.stderr()));
    assert_eq!(surface.kind, WindowKind::Lock);
    wait_info(&s, WindowRef::Lock, |w| w.focused);
    wait_pixel(&s, 640, 20, [64, 255, 64]);
    s.blocking_key("a").expect("key");
    wait_event(&second, "unlocked", |e| is(e, "unlocked"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_live_lock_refuses_a_second_lock_and_yields_to_force_unlock() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.force");
    let _bar = bar(&s);
    let holder = lock(&s, &["--color", "#4040ff"]);
    wait_pixel(&s, 640, 20, [64, 64, 255]);

    let refused = s
        .blocking_launch(layer(&["--mode", "lock"]))
        .expect("launch second lock");
    wait_event(&refused, "finished", |e| is(e, "finished"));
    assert_eq!(count_events(&refused, |e| is(e, "locked")), 0);

    assert!(s.blocking_force_unlock().expect("force unlock"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    assert!(!s.blocking_force_unlock().expect("second force unlock"));
    assert_eq!(count_events(&holder, |e| is(e, "finished")), 0);

    // The stale lock client still runs, and a new lock takes the session.
    let next = lock(&s, &["--unlock-on-input", "--color", "#40ff40"]);
    wait_pixel(&s, 640, 20, [64, 255, 64]);
    click(&s, 640.0, 400.0);
    wait_event(&next, "unlocked", |e| is(e, "unlocked"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_relock_before_the_old_lock_surfaces_go_keeps_the_session_locked() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.relock-now");
    let _bar = bar(&s);
    let lock = lock(
        &s,
        &[
            "--relock-on-input",
            "--unlock-on-input",
            "--color",
            "#4040ff",
        ],
    );
    wait_info(&s, WindowRef::Lock, |w| w.focused);

    // The client unlocks and locks again in one flush: the second lock holds the session.
    s.blocking_key("a").expect("key");
    wait_event(&lock, "relocked", |e| is(e, "relocked"));
    wait_count(&lock, "locked", 2);
    assert_eq!(count_events(&lock, |e| is(e, "finished")), 0);
    wait_event(&lock, "new lock surface focus", |e| {
        is(e, "keyboard_enter") && e["surface"] == "lock1"
    });
    wait_pixel(&s, 640, 20, [64, 64, 255]);
    let w = s
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert!(!w.focused, "the toplevel stays out of reach");

    // The next key reaches the new lock surface and unlocks.
    s.blocking_key("a").expect("key");
    wait_event(&lock, "unlocked", |e| is(e, "unlocked"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    assert!(!s.blocking_force_unlock().expect("force unlock"));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_lock_after_a_forced_unlock_and_a_killed_client_unlocks() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.recover");
    let _bar = bar(&s);
    let hung = lock(&s, &["--color", "#4040ff"]);
    wait_pixel(&s, 640, 20, [64, 64, 255]);
    assert!(s.blocking_force_unlock().expect("force unlock"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    hung.kill().expect("kill");
    hung.blocking_wait();

    let next = lock(&s, &["--unlock-on-input", "--color", "#40ff40"]);
    wait_info(&s, WindowRef::Lock, |w| w.focused);
    wait_pixel(&s, 640, 20, [64, 255, 64]);
    s.blocking_key("a").expect("key");
    wait_event(&next, "unlocked", |e| is(e, "unlocked"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    wait_info(&s, WindowRef::id(window.id), |w| w.focused);
    assert!(
        !s.blocking_force_unlock().expect("force unlock"),
        "the unlock of the next lock leaves the session locked"
    );
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn the_unlock_of_a_force_unlocked_client_leaves_a_newer_lock_in_place() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let (_top, window) = toplevel(&s, "org.example.stale");
    let _bar = bar(&s);
    let stale = lock(&s, &["--color", "#4040ff", "--unlock-after-ms", "4000"]);
    assert!(s.blocking_force_unlock().expect("force unlock"));
    let next = lock(&s, &["--unlock-on-input", "--color", "#40ff40"]);
    wait_pixel(&s, 640, 20, [64, 255, 64]);
    assert_eq!(
        count_events(&stale, |e| is(e, "unlocked")),
        0,
        "the stale unlock came before the next lock"
    );

    // The stale client unlocks and exits. The next lock still holds the session.
    wait_event(&stale, "stale unlock", |e| is(e, "unlocked"));
    stale.blocking_wait();
    assert!(
        !stale.stderr().contains("error"),
        "the stale client hit an error: {}",
        stale.stderr()
    );
    assert_eq!(pixel_at(&s, 640, 20), [64, 255, 64]);
    let w = s
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert!(!w.focused, "the toplevel stays out of reach");

    s.blocking_key("a").expect("key");
    wait_event(&next, "unlocked", |e| is(e, "unlocked"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    assert!(!s.blocking_force_unlock().expect("force unlock"));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_force_unlocked_client_locks_the_same_output_again() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let _bar = bar(&s);
    let holder = lock(&s, &["--color", "#4040ff", "--lock-again-after-ms", "2000"]);
    assert!(s.blocking_force_unlock().expect("force unlock"));
    assert_eq!(
        count_events(&holder, |e| is(e, "lock_again")),
        0,
        "the second lock came before the forced unlock"
    );
    wait_pixel(&s, 640, 20, [255, 0, 0]);

    // The second lock of the same client covers the same output with a new lock surface.
    wait_count(&holder, "locked", 2);
    assert_eq!(count_events(&holder, |e| is(e, "finished")), 0);
    wait_pixel(&s, 640, 20, [64, 64, 255]);
    assert!(
        !holder.stderr().contains("error"),
        "the lock client hit an error: {}",
        holder.stderr()
    );
    assert!(s.blocking_force_unlock().expect("force unlock"));
    s.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the layer client"]
fn a_second_lock_of_the_holder_is_refused_without_an_error() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let _bar = bar(&s);
    let holder = lock(&s, &["--color", "#4040ff", "--lock-again-after-ms", "200"]);
    wait_pixel(&s, 640, 20, [64, 64, 255]);

    // The second lock asks for a lock surface on the same output and gets `finished`.
    wait_event(&holder, "finished", |e| is(e, "finished"));
    holder.blocking_wait();
    assert!(
        !holder.stderr().contains("error"),
        "the lock client hit an error: {}",
        holder.stderr()
    );
    assert_eq!(count_events(&holder, |e| is(e, "locked")), 1);
    assert_eq!(pixel_at(&s, 640, 20), [0, 0, 0], "the session stays locked");
    assert!(s.blocking_force_unlock().expect("force unlock"));
    wait_pixel(&s, 640, 20, [255, 0, 0]);
    s.blocking_shutdown().expect("shutdown");
}

/// Waits until the session lists no lock surface.
fn poll_no_lock_surface(s: &Session) {
    support::shell::poll(
        || "the lock surfaces stay".to_owned(),
        || {
            let windows = s.blocking_windows().expect("windows");
            windows
                .iter()
                .all(|w| w.kind != WindowKind::Lock)
                .then_some(())
        },
    );
}
