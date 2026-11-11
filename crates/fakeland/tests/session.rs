//! Session lifecycle without GPU clients: start, socket, outputs, hotplug, traces, subscriptions,
//! waits and shutdown. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use std::path::PathBuf;

use fakeland::{
    CaptureOptions, CaptureTarget, Event, Fakeland, ModeConfig, OutputChange, OutputConfig,
    RendererChoice, SessionError, SubscriptionItem, WindowRef,
};
use fakeland_types::{Hz, Nanos};

fn socket_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR")).join(name)
}

#[test]
#[ignore = "starts a compositor session"]
fn session_starts_records_and_shuts_down() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .renderer(RendererChoice::Pixman)
        .start()
        .expect("start");
    let socket = socket_path(session.socket_name());
    assert!(socket.exists(), "socket {} exists", socket.display());
    assert!(session.socket_name().starts_with("wayland-"));
    assert!(session.dbus_address().is_some());
    assert_eq!(session.environment().renderer, "pixman");

    let mut sub = session.blocking_subscribe(1024).expect("subscribe");
    let capture = session.blocking_begin_capture().expect("capture");
    session.blocking_mark("hello").expect("mark");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let trace = session.blocking_end_capture(capture).expect("end capture");
    let vblanks = support::count(&trace, |e| matches!(e, Event::Vblank { .. }));
    assert!(
        (8..=16).contains(&vblanks),
        "{vblanks} vblanks in 200 ms at 60 Hz"
    );
    assert_eq!(
        support::count(
            &trace,
            |e| matches!(e, Event::UserMark { label } if label == "hello")
        ),
        1
    );
    let mut got_mark = false;
    while let Ok(Some(item)) = sub.try_next() {
        if let SubscriptionItem::Record(r) = item
            && matches!(&r.event, Event::UserMark { label } if label == "hello")
        {
            got_mark = true;
        }
    }
    assert!(got_mark, "the subscription delivers the mark");

    let validity = session.blocking_validity().expect("validity");
    assert!(validity.wakes >= 8);

    let shot = session
        .blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())
        .expect("screenshot");
    assert_eq!((shot.image.width(), shot.image.height()), (1280, 720));
    assert_eq!(shot.image.pixel(10, 10), Some([0, 0, 0, 255]));

    let err = session
        .blocking_wait_window(WindowRef::Any, Nanos::from_millis(50))
        .unwrap_err();
    assert!(matches!(err, SessionError::Timeout), "{err}");

    session.blocking_shutdown().expect("shutdown");
    assert!(!socket.exists(), "socket removed");
    let mut left = 100_000;
    while sub.blocking_next().is_some() {
        left -= 1;
        assert!(left > 0, "the subscription ends");
    }
    assert!(matches!(
        session.blocking_mark("late"),
        Err(SessionError::Closed)
    ));
}

#[test]
#[ignore = "starts a compositor session"]
fn outputs_change_and_hotplug() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1920, 1080, Hz(60.0)))
        .renderer(RendererChoice::Pixman)
        .start()
        .expect("start");
    let outputs = session.blocking_outputs().expect("outputs");
    assert_eq!(outputs.len(), 1);
    let first = outputs[0].id;

    session
        .blocking_set_output(
            None,
            OutputChange::Mode(ModeConfig::new(2560, 1440, Hz(144.0))),
        )
        .expect("mode");
    session
        .blocking_set_output(None, OutputChange::Scale { scale: 2.0 })
        .expect("scale");
    let out = &session.blocking_outputs().expect("outputs")[0];
    assert_eq!(out.mode.width, 2560);
    assert_eq!(out.logical_size.w, 1280);

    let second = session
        .blocking_add_output(OutputConfig::new("HDMI-A-1").mode(1280, 1024, Hz(75.0)))
        .expect("hotplug");
    let outputs = session.blocking_outputs().expect("outputs");
    assert_eq!(outputs.len(), 2);
    assert_eq!(
        outputs[1].position.x, 1280,
        "placed right of the first output"
    );

    session
        .blocking_set_output(Some(second), OutputChange::Disable)
        .expect("disable");
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_millis(100));
    let trace = session.blocking_end_capture(capture).expect("trace");
    assert_eq!(
        support::count(
            &trace,
            |e| matches!(e, Event::Vblank { output, .. } if *output == second)
        ),
        0,
        "a disabled output has no vblanks"
    );
    assert!(
        support::count(
            &trace,
            |e| matches!(e, Event::Vblank { output, .. } if *output == first)
        ) >= 10
    );

    session.blocking_remove_output(second).expect("unplug");
    assert_eq!(session.blocking_outputs().expect("outputs").len(), 1);
    assert!(
        session.blocking_remove_output(first).is_err(),
        "the last output stays"
    );
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "starts a compositor session"]
fn bad_configs_fail_to_start() {
    let mut config = fakeland::SessionConfig::default();
    config.outputs.clear();
    assert!(Fakeland::start(config).is_err());
}

#[test]
#[ignore = "starts a compositor session"]
fn children_get_an_isolated_environment_and_die_with_the_session() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .renderer(RendererChoice::Pixman)
        .env("SESSION_EXTRA", "on")
        .start()
        .expect("start");
    let env = session
        .blocking_launch(
            fakeland::LaunchSpec::from_argv(["sh", "-c", "env; echo oops >&2"])
                .expect("argv")
                .env("LAUNCH_EXTRA", "yes"),
        )
        .expect("launch");
    let exit = env.blocking_wait();
    assert!(exit.success());
    let out = env.stdout();
    assert!(out.contains(&format!("WAYLAND_DISPLAY={}\n", session.socket_name())));
    assert!(!out.contains("\nDISPLAY="), "no X display");
    assert!(!out.contains("WAYLAND_SOCKET="));
    assert!(out.contains("XDG_SESSION_TYPE=wayland"));
    assert!(out.contains("GDK_BACKEND=wayland"));
    assert!(out.contains("QT_QPA_PLATFORM=wayland"));
    assert!(out.contains("SESSION_EXTRA=on"));
    assert!(out.contains("LAUNCH_EXTRA=yes"));
    let bus = session.dbus_address().expect("private bus");
    assert!(out.contains(&format!("DBUS_SESSION_BUS_ADDRESS={bus}")));
    let added = [
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "GDK_BACKEND",
        "QT_QPA_PLATFORM",
        "SDL_VIDEODRIVER",
        "ELECTRON_OZONE_PLATFORM_HINT",
        "MOZ_ENABLE_WAYLAND",
        "DBUS_SESSION_BUS_ADDRESS",
        "GTK_A11Y",
        "QT_LINUX_ACCESSIBILITY_ALWAYS_ON",
        "SESSION_EXTRA",
        "LAUNCH_EXTRA",
    ];
    for line in out.lines() {
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let name_like =
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !name_like {
            continue;
        }
        let inherited = std::env::var_os(key).is_some();
        assert!(
            inherited || added.contains(&key) || ["PWD", "SHLVL", "_"].contains(&key),
            "the session set {key}"
        );
    }
    assert_eq!(env.stderr(), "oops\n");

    let sleeper = session
        .blocking_launch(fakeland::LaunchSpec::from_argv(["sleep", "30"]).expect("argv"))
        .expect("launch");
    let killed = session
        .blocking_launch(fakeland::LaunchSpec::from_argv(["sleep", "30"]).expect("argv"))
        .expect("launch");
    killed.kill().expect("kill");
    assert_eq!(killed.blocking_wait().signal, Some(9));
    std::thread::sleep(std::time::Duration::from_millis(50));
    let trace = session.blocking_trace_snapshot().expect("trace");
    assert_eq!(
        support::count(&trace, |e| matches!(e, Event::ProcessSpawned { .. })),
        3
    );
    assert!(
        support::count(
            &trace,
            |e| matches!(e, Event::ProcessExited { pid, signal: Some(9), .. } if *pid == killed.pid())
        ) == 1
    );

    assert!(matches!(
        session.blocking_launch(fakeland::LaunchSpec::new("/nonexistent/program")),
        Err(SessionError::Launch(_))
    ));
    let launcher = |secs: &str| {
        let app = session
            .blocking_launch(
                fakeland::LaunchSpec::from_argv(["sh", "-c", &format!("sleep {secs} & echo $!")])
                    .expect("argv"),
            )
            .expect("launch");
        assert!(app.blocking_wait().success());
        let pid: u32 = app.stdout().trim().parse().expect("background pid");
        assert!(alive(pid), "the background child outlives its launcher");
        (app, pid)
    };
    let (orphaned, killed_pid) = launcher("31");
    orphaned.kill().expect("kill the group of an exited leader");
    assert!(
        gone(killed_pid),
        "kill reaches the group after the leader exits"
    );
    let (_left, left_pid) = launcher("32");

    session.blocking_shutdown().expect("shutdown");
    assert!(
        sleeper.try_wait().is_some(),
        "children end with the session"
    );
    assert!(
        gone(left_pid),
        "the group of an exited leader ends with the session"
    );
}

/// Returns true while a process runs. A zombie counts as ended.
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            s.rsplit_once(')')
                .map(|(_, rest)| rest.trim_start().chars().next())
        })
        .flatten()
        .is_some_and(|state| state != 'Z')
}

/// Waits up to two seconds for a process to end.
fn gone(pid: u32) -> bool {
    for _ in 0..200 {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

#[test]
#[ignore = "starts a compositor session"]
fn bus_activated_services_see_the_session_display() {
    const STORE: &str = "/usr/lib/xdg-permission-store";
    if !std::path::Path::new(STORE).exists() || !support::have("gdbus") {
        eprintln!("skipped: needs {STORE} and gdbus");
        return;
    }
    let _serial = support::serial();
    let session = Fakeland::builder()
        .renderer(RendererChoice::Pixman)
        .accessibility(false)
        .start()
        .expect("start");
    let script = String::from(
        "gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore \
         --object-path /org/freedesktop/impl/portal/PermissionStore \
         --method org.freedesktop.DBus.Peer.Ping >/dev/null && \
         for p in $(pgrep -f xdg-permission-store); do \
           [ \"$(readlink /proc/$p/exe)\" = /usr/lib/xdg-permission-store ] || continue; \
           if grep -qz \"^DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS$\" /proc/$p/environ; then \
             echo cwd=$(readlink /proc/$p/cwd); tr '\\0' '\\n' < /proc/$p/environ; fi; \
         done; true",
    );
    let app = session
        .blocking_launch(fakeland::LaunchSpec::from_argv(["sh", "-c", &script]).expect("argv"))
        .expect("launch");
    assert!(app.blocking_wait().success(), "{}", app.stderr());
    let out = app.stdout();
    assert!(out.contains("cwd=/\n"), "activated service env:\n{out}");
    assert!(out.contains(&format!("\nWAYLAND_DISPLAY={}\n", session.socket_name())));
    for host in ["DISPLAY=", "HYPRLAND_INSTANCE_SIGNATURE=", "SWAYSOCK="] {
        assert!(!out.contains(&format!("\n{host}")), "{host} leaks: {out}");
    }
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "starts a compositor session"]
fn clipboard_cursor_and_every_frame_composite() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(640, 480, Hz(60.0)))
        .renderer(RendererChoice::Pixman)
        .composite(fakeland::Composite::EveryFrame)
        .start()
        .expect("start");
    session
        .blocking_set_clipboard("text/plain;charset=utf-8", "copied text")
        .expect("set clipboard");
    assert_eq!(
        session
            .blocking_clipboard_text(Nanos::from_millis(500))
            .expect("read clipboard"),
        "copied text"
    );
    let shot = session
        .blocking_screenshot(
            CaptureTarget::default(),
            CaptureOptions {
                include_cursor: true,
                max_dim: None,
            },
        )
        .expect("screenshot");
    assert_eq!(
        shot.image.pixel(0, 0),
        Some([255, 255, 255, 255]),
        "cursor outline"
    );
    assert_eq!(shot.image.pixel(1, 2), Some([0, 0, 0, 255]), "cursor fill");
    assert_eq!(shot.image.pixel(40, 40), Some([0, 0, 0, 255]));
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let trace = session.blocking_end_capture(capture).expect("trace");
    let composites = support::count(
        &trace,
        |e| matches!(e, Event::CompositorOverhead { what, .. } if what == "composite"),
    );
    assert!(composites >= 8, "{composites} composites in 200 ms");
    assert!(support::count(&trace, |e| matches!(e, Event::Clipboard { .. })) <= 1);
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "starts a compositor session"]
fn dropping_the_last_handle_shuts_down() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .renderer(RendererChoice::Pixman)
        .start()
        .expect("start");
    let socket = socket_path(session.socket_name());
    let clone = session.clone();
    let sleeper = session
        .blocking_launch(fakeland::LaunchSpec::from_argv(["sleep", "30"]).expect("argv"))
        .expect("launch");
    drop(session);
    assert!(socket.exists(), "a clone keeps the session alive");
    std::thread::spawn(move || drop(clone))
        .join()
        .expect("drop on another thread");
    assert!(!socket.exists(), "the socket goes with the last handle");
    assert!(
        sleeper.try_wait().is_some(),
        "children end with the session"
    );
    assert!(matches!(
        sleeper.blocking_windows(),
        Err(SessionError::Closed)
    ));
}
