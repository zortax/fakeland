//! End-to-end test of the accessibility methods over the control socket with a GTK dialog: the
//! tree with short node names, a search, a click through the action, and a text change.

use std::time::Duration;

use fakeland::{A11yTarget, ClickVia, Fakeland, LaunchSpec, RendererChoice, SessionConfig};
use fakeland_control::methods::*;
use fakeland_control::wire::ErrorCode;
use fakeland_control::{ClientError, ControlClient, serve_session};
use fakeland_scenario::WindowRef;
use fakeland_types::Dur;

fn have(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// Searches until a match shows up, at most 10 s.
fn find(c: &ControlClient, p: &A11yFindParams) -> A11yFound {
    for _ in 0..100 {
        let f = c.blocking_call::<A11yFind>(p.clone()).expect("find");
        if !f.nodes.is_empty() {
            return f;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("nothing matched {p:?}");
}

#[test]
#[ignore = "starts a session; run with `just it`"]
fn drives_a_gtk_dialog_through_the_accessibility_tree() {
    if !have("zenity") {
        eprintln!("zenity is missing");
        return;
    }
    let config = SessionConfig {
        renderer: RendererChoice::Pixman,
        ..SessionConfig::default()
    };
    let session = Fakeland::start(config).expect("session");
    let dir = tempfile::Builder::new()
        .prefix("flctl")
        .tempdir_in("/tmp")
        .unwrap();
    let path = dir.path().join("c.sock");
    let server = serve_session(session, Some(&path)).expect("serve");
    let c = ControlClient::connect(&path).expect("connect");

    let app = c
        .blocking_call::<Launch>(LaunchParams {
            spec: LaunchSpec::new("zenity").args(["--entry", "--text", "Your name"]),
            wait_window: Some(Dur::from_secs(20)),
        })
        .unwrap();
    let window = app.window.expect("window");
    let target = A11yTarget::Window {
        window: WindowRef::Id { id: window.id },
    };
    let entry = find(
        &c,
        &A11yFindParams {
            target: target.clone(),
            query: fakeland::accessibility::Query::default().with_state("editable"),
            ..A11yFindParams::default()
        },
    );
    assert!(entry.text.starts_with("- "), "{}", entry.text);
    let short = entry
        .text
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("short id")
        .to_string();
    assert!(short.starts_with('n'), "{short}");

    let tree = c
        .blocking_call::<A11yTreeOf>(A11yTreeParams {
            target: target.clone(),
            ..A11yTreeParams::default()
        })
        .unwrap();
    assert!(tree.text.contains(&format!("#{short} ")), "{}", tree.text);
    assert!(tree.text.contains("button \"OK\""), "{}", tree.text);
    assert_eq!(tree.tree.window, Some(window.id));

    let apps = c.blocking_call::<A11yApps>(NoParams {}).unwrap();
    assert!(
        apps.iter()
            .any(|a| a.name == "zenity" && a.frames.iter().any(|f| f.window == Some(window.id)))
    );

    let node = c
        .blocking_call::<A11yNode>(A11yNodeParams {
            node: short.clone(),
        })
        .unwrap();
    assert_eq!(node.id, entry.nodes[0].id);

    let set = c
        .blocking_call::<A11ySetText>(A11ySetTextParams {
            node: short.clone(),
            text: "over the socket".into(),
        })
        .unwrap();
    eprintln!("set text {:?}", set.method);

    let err = c.blocking_call::<A11yAction>(A11yActionParams {
        node: "n99999".into(),
        action: "click".into(),
        via: ClickVia::Auto,
    });
    assert!(
        matches!(err, Err(ClientError::Remote(ref b)) if b.code == ErrorCode::InvalidParams),
        "{err:?}"
    );

    let ok = find(
        &c,
        &A11yFindParams {
            target: target.clone(),
            query: fakeland::accessibility::Query::role("button").with_name("^ok$"),
            ..A11yFindParams::default()
        },
    );
    let acted = c
        .blocking_call::<A11yAction>(A11yActionParams {
            node: ok.nodes[0].id.to_string(),
            action: "click".into(),
            via: ClickVia::Action,
        })
        .unwrap();
    assert_eq!(acted.via, Some(ClickVia::Action));

    let logs = (0..50)
        .find_map(|_| {
            let l = c
                .blocking_call::<AppLogs>(LogsParams {
                    pid: Some(app.pid),
                    lines: 10,
                })
                .ok()?;
            if l.stdout.is_empty() {
                std::thread::sleep(Duration::from_millis(100));
                None
            } else {
                Some(l)
            }
        })
        .expect("output");
    assert_eq!(logs.stdout.trim_end(), "over the socket");

    c.blocking_call::<Shutdown>(NoParams {}).unwrap();
    server.wait();
}
