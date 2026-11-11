//! Accessibility through the session: the private accessibility bus and registry, the trees of
//! GTK and Chromium apps with bounds in the global logical space, clicks through actions and
//! through the pointer, text changes, and teardown with the session.

mod support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use fakeland::accessibility::{Node, Query, TreeOptions, bus_status};
use fakeland::{
    A11yTarget, Accessibility, ClickVia, Fakeland, LaunchSpec, OutputConfig, RendererChoice,
    Session, TextMethod, WindowRef,
};
use fakeland_types::{Hz, Nanos, Rect};

fn session(scale: f64) -> Session {
    Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1280, 800, Hz(60.0))
                .scale(scale),
        )
        .renderer(RendererChoice::Auto)
        .start()
        .expect("start")
}

/// Polls `f` every 100 ms until it returns a value, at most `tries` times.
fn eventually<T>(tries: u32, mut f: impl FnMut() -> Option<T>) -> T {
    for _ in 0..tries {
        if let Some(v) = f() {
            return v;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("the condition did not hold in {} ms", tries * 100);
}

fn find_one(a: &Accessibility, target: &A11yTarget, q: &Query) -> Option<Node> {
    a.blocking_find(target, q, &TreeOptions::default().showing(true), 1)
        .ok()?
        .into_iter()
        .next()
}

/// Returns the pids whose environment holds `XDG_RUNTIME_DIR=dir`.
fn pids_with_runtime_dir(dir: &str) -> Vec<u32> {
    let wanted = format!("XDG_RUNTIME_DIR={dir}");
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(env) = std::fs::read(entry.path().join("environ")) else {
            continue;
        };
        if env.split(|b| *b == 0).any(|kv| kv == wanted.as_bytes()) {
            out.push(pid);
        }
    }
    out
}

/// Returns the runtime directory of an accessibility bus address.
fn runtime_dir_of(address: &str) -> String {
    let path = address
        .strip_prefix("unix:path=")
        .and_then(|rest| rest.split(',').next())
        .expect("unix path address");
    Path::new(path)
        .parent()
        .and_then(Path::parent)
        .expect("runtime dir")
        .to_string_lossy()
        .into_owned()
}

#[test]
#[ignore = "starts a compositor session"]
fn private_bus_runs_and_ends_with_the_session() {
    let _serial = support::serial();
    let s = session(1.0);
    let a11y = s.blocking_a11y().expect("a11y");
    let address = a11y.client().address().to_string();
    let runtime = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR");
    let dir = runtime_dir_of(&address);
    assert!(dir.starts_with(&runtime), "{dir}");
    assert!(!dir.to_lowercase().contains("fakeland"));
    assert_ne!(address, "unix:path=/run/user/1000/at-spi/bus");
    let status =
        fakeland::block_on(bus_status(s.dbus_address().expect("private bus"))).expect("status");
    assert!(status.enabled);
    assert!(!status.screen_reader);
    assert!(a11y.blocking_apps().expect("apps").is_empty());
    let pids = pids_with_runtime_dir(&dir);
    assert!(
        pids.len() >= 3,
        "launcher, accessibility bus and registry: {pids:?}"
    );

    let env = s
        .blocking_launch(LaunchSpec::from_argv(["sh", "-c", "env"]).expect("argv"))
        .expect("launch");
    env.blocking_wait();
    let out = env.stdout();
    assert!(out.contains("GTK_A11Y=atspi\n"));
    assert!(out.contains("QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1\n"));
    assert!(!out.contains("NO_AT_BRIDGE="));
    assert!(!out.contains("AT_SPI_BUS_ADDRESS="));

    s.blocking_shutdown().expect("shutdown");
    let left = eventually(30, || {
        let left = pids_with_runtime_dir(&dir);
        left.is_empty().then_some(left)
    });
    assert!(left.is_empty());
    assert!(!Path::new(&dir).exists(), "the runtime dir goes away");
}

#[test]
#[ignore = "starts a compositor session"]
fn accessibility_off_turns_the_bridges_off() {
    let _serial = support::serial();
    let s = Fakeland::builder()
        .renderer(RendererChoice::Pixman)
        .accessibility(false)
        .start()
        .expect("start");
    assert!(s.blocking_a11y().is_err());
    let env = s
        .blocking_launch(LaunchSpec::from_argv(["sh", "-c", "env"]).expect("argv"))
        .expect("launch");
    env.blocking_wait();
    let out = env.stdout();
    assert!(out.contains("NO_AT_BRIDGE=1\n"));
    assert!(out.contains("GTK_A11Y=none\n"));
    s.blocking_shutdown().expect("shutdown");
}

/// Checks that a node lies inside a window and has a sensible size.
fn inside(node: &Node, window: Rect<fakeland_types::Logical>) {
    let b = node
        .bounds
        .unwrap_or_else(|| panic!("{} has no bounds", node.id));
    assert!(b.w > 0 && b.h > 0, "{b:?}");
    assert!(
        b.x >= window.x
            && b.y >= window.y
            && b.x + b.w <= window.x + window.w
            && b.y + b.h <= window.y + window.h,
        "{b:?} lies outside {window:?}"
    );
}

#[test]
#[ignore = "starts a compositor session"]
fn gtk_dialog_tree_actions_and_text() {
    if !support::have("zenity") {
        eprintln!("zenity is missing");
        return;
    }
    let _serial = support::serial();
    let s = session(1.5);
    let a11y = s.blocking_a11y().expect("a11y");

    // A question dialog: the tree, then Yes through its action.
    let app = s
        .blocking_launch(
            LaunchSpec::from_argv(["zenity", "--question", "--text", "Proceed?"]).expect("argv"),
        )
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(20))
        .expect("window");
    let target = A11yTarget::Window {
        window: WindowRef::Id { id: window.id },
    };
    let yes = eventually(100, || {
        find_one(&a11y, &target, &Query::role("button").with_name("^yes$"))
    });
    let tree = a11y
        .blocking_tree(&target, &TreeOptions::default().showing(true))
        .expect("tree");
    let text = a11y.client().render(&tree, &Default::default());
    eprintln!("{text}");
    assert_eq!(tree.window, Some(window.id));
    assert_eq!(
        tree.bounds,
        Some(window.geometry),
        "the frame covers the window"
    );
    assert!(text.contains("label \"Proceed?\""), "{text}");
    assert!(text.contains("button \"No\""), "{text}");
    inside(&yes, window.geometry);
    let single = a11y.blocking_node(&yes.id).expect("node");
    assert_eq!(
        single.bounds, yes.bounds,
        "a single node maps like the tree"
    );
    let label =
        find_one(&a11y, &target, &Query::role("label").with_name("Proceed")).expect("label");
    inside(&label, window.geometry);
    let out = a11y
        .blocking_click(&yes.id, ClickVia::Action)
        .expect("click");
    assert_eq!(out.via, ClickVia::Action);
    assert_eq!(
        out.action.as_deref().map(str::to_lowercase).as_deref(),
        Some("click")
    );
    assert_eq!(app.blocking_wait().code, Some(0), "Yes answers 0");

    // The same dialog: No through a real pointer click at the center of its bounds.
    let app = s
        .blocking_launch(
            LaunchSpec::from_argv(["zenity", "--question", "--text", "Proceed?"]).expect("argv"),
        )
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(20))
        .expect("window");
    let target = A11yTarget::Window {
        window: WindowRef::Id { id: window.id },
    };
    let no = eventually(100, || {
        find_one(&a11y, &target, &Query::role("button").with_name("^no$"))
    });
    s.blocking_wait_settled(
        WindowRef::Id { id: window.id },
        Nanos::from_millis(300),
        Nanos::from_secs(5),
    )
    .expect("settled");
    let out = a11y
        .blocking_click(&no.id, ClickVia::Pointer)
        .expect("click");
    assert_eq!(out.via, ClickVia::Pointer);
    assert!(!out.sent.is_empty());
    assert_eq!(app.blocking_wait().code, Some(1), "No answers 1");

    // An entry dialog: replace the text, then OK prints it.
    let app = s
        .blocking_launch(
            LaunchSpec::from_argv([
                "zenity",
                "--entry",
                "--text",
                "Your name",
                "--entry-text",
                "abc",
            ])
            .expect("argv"),
        )
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(20))
        .expect("window");
    let target = A11yTarget::Window {
        window: WindowRef::Id { id: window.id },
    };
    let entry = eventually(100, || {
        find_one(
            &a11y,
            &target,
            &Query::default().with_state("editable").with_text("abc"),
        )
    });
    assert_eq!(entry.text.as_deref(), Some("abc"));
    let how = a11y
        .blocking_set_text(&entry.id, "Grüße from a11y")
        .expect("set text");
    eprintln!("set text {how:?}");
    let entry = a11y.blocking_node(&entry.id).expect("node");
    assert_eq!(entry.text.as_deref(), Some("Grüße from a11y"));
    let ok = find_one(&a11y, &target, &Query::role("button").with_name("^ok$")).expect("ok");
    a11y.blocking_click(&ok.id, ClickVia::Auto).expect("click");
    assert_eq!(app.blocking_wait().code, Some(0));
    assert_eq!(app.stdout(), "Grüße from a11y\n");
    if how == TextMethod::Typed {
        eprintln!("the entry has no EditableText interface");
    }
    s.blocking_shutdown().expect("shutdown");
}

fn chrome_page(dir: &Path) -> PathBuf {
    let page = dir.join("a11y-page.html");
    std::fs::write(
        &page,
        r#"<!doctype html><html><head><title>A11y page</title></head>
<body style="margin:0;font:16px sans-serif">
<h1 style="margin:20px">Hello page</h1>
<button style="position:absolute;left:100px;top:120px;width:200px;height:50px"
  onclick="document.getElementById('out').textContent='clicked '+(window.n=(window.n||0)+1)">Press me</button>
<input aria-label="Your name" style="position:absolute;left:100px;top:200px;width:300px;height:30px">
<p id="out" style="position:absolute;left:100px;top:260px">not clicked</p>
</body></html>"#,
    )
    .expect("page");
    page
}

#[test]
#[ignore = "starts a compositor session"]
fn chromium_page_tree_clicks_and_text() {
    if !support::have("google-chrome-stable") {
        eprintln!("google-chrome-stable is missing");
        return;
    }
    let _serial = support::serial();
    for scale in [1.0, 1.5] {
        let s = session(scale);
        let a11y = s.blocking_a11y().expect("a11y");
        let dir = support::out_dir();
        let page = chrome_page(&dir);
        let profile = dir.join(format!("chrome-a11y-profile-{scale}"));
        let _ = std::fs::remove_dir_all(&profile);
        let app = s
            .blocking_launch(
                LaunchSpec::new("google-chrome-stable")
                    .arg("--ozone-platform=wayland")
                    .arg("--force-renderer-accessibility")
                    .arg(format!("--user-data-dir={}", profile.display()))
                    .arg("--no-first-run")
                    .arg("--no-default-browser-check")
                    .arg(format!("file://{}", page.display())),
            )
            .expect("launch");
        let window = app
            .blocking_wait_window(Nanos::from_secs(30))
            .expect("window");
        let target = A11yTarget::Pid { pid: app.pid() };
        let button = eventually(200, || {
            find_one(
                &a11y,
                &target,
                &Query::role("button").with_name("^press me$"),
            )
            .filter(|n| n.bounds.is_some())
        });
        let doc = find_one(
            &a11y,
            &target,
            &Query::role("document web").with_name("A11y page"),
        )
        .expect("document");
        let (b, d) = (button.bounds.expect("bounds"), doc.bounds.expect("bounds"));
        eprintln!(
            "scale {scale}: button {b:?} document {d:?} window {:?}",
            window.geometry
        );
        assert_eq!(button.window, Some(window.id));
        assert!((b.x - d.x - 100).abs() <= 1, "{b:?} {d:?}");
        assert!((b.y - d.y - 120).abs() <= 1, "{b:?} {d:?}");
        assert!((b.w - 200).abs() <= 1 && (b.h - 50).abs() <= 1, "{b:?}");
        inside(&button, window.geometry);
        let single = a11y.blocking_node(&button.id).expect("node");
        assert_eq!(
            single.bounds, button.bounds,
            "a single node maps like the tree"
        );
        assert_eq!(single.window, Some(window.id));

        let out = a11y
            .blocking_click(&button.id, ClickVia::Action)
            .expect("click");
        assert_eq!(out.via, ClickVia::Action);
        eventually(50, || {
            find_one(&a11y, &target, &Query::default().with_name("^clicked 1$"))
        });
        s.blocking_wait_settled(
            WindowRef::Id { id: window.id },
            Nanos::from_millis(200),
            Nanos::from_secs(5),
        )
        .ok();
        let out = a11y
            .blocking_click(&button.id, ClickVia::Pointer)
            .expect("click");
        assert_eq!(out.via, ClickVia::Pointer);
        eventually(50, || {
            find_one(&a11y, &target, &Query::default().with_name("^clicked 2$"))
        });

        let entry =
            find_one(&a11y, &target, &Query::role("entry").with_name("your name")).expect("entry");
        let how = a11y
            .blocking_set_text(&entry.id, "typed by a11y")
            .expect("set text");
        eprintln!("chrome set text {how:?}");
        let value = eventually(50, || {
            let n = a11y.blocking_node(&entry.id).ok()?;
            (n.text.as_deref() == Some("typed by a11y")).then_some(n)
        });
        assert_eq!(value.text.as_deref(), Some("typed by a11y"));
        app.terminate().ok();
        app.blocking_wait();
        s.blocking_shutdown().expect("shutdown");
    }
}

#[test]
#[ignore = "starts a compositor session"]
fn qt_dialog_bounds_and_pointer_click() {
    if !support::have("designer6") {
        eprintln!("designer6 is missing");
        return;
    }
    let _serial = support::serial();
    let s = session(1.0);
    let a11y = s.blocking_a11y().expect("a11y");
    let app = s
        .blocking_launch(LaunchSpec::new("designer6"))
        .expect("launch");
    app.blocking_wait_window(Nanos::from_secs(30))
        .expect("window");
    let dialog = eventually(200, || {
        s.blocking_windows()
            .ok()?
            .into_iter()
            .find(|w| w.is_mapped() && w.title.starts_with("New Form"))
    });
    let target = A11yTarget::Window {
        window: WindowRef::Id { id: dialog.id },
    };
    let close = eventually(100, || {
        find_one(&a11y, &target, &Query::role("button").with_name("^close$"))
    });
    let create =
        find_one(&a11y, &target, &Query::role("button").with_name("^create$")).expect("create");
    inside(&close, dialog.geometry);
    inside(&create, dialog.geometry);
    let (c, k) = (
        close.bounds.expect("bounds"),
        create.bounds.expect("bounds"),
    );
    assert_eq!(c.y, k.y, "the buttons share a row");
    assert!(k.x < c.x, "Create sits left of Close");
    s.blocking_wait_settled(
        WindowRef::Id { id: dialog.id },
        Nanos::from_millis(300),
        Nanos::from_secs(5),
    )
    .ok();
    let out = a11y
        .blocking_click(&close.id, ClickVia::Pointer)
        .expect("click");
    assert_eq!(out.via, ClickVia::Pointer);
    eventually(50, || {
        let open = s
            .blocking_windows()
            .ok()?
            .into_iter()
            .any(|w| w.id == dialog.id);
        (!open).then_some(())
    });
    app.terminate().ok();
    app.blocking_wait();
    s.blocking_shutdown().expect("shutdown");
}
