//! Input injection against the reference client: scroll glides by wheel and finger, a classic
//! wheel device, typed text through keys, shift levels, text input and a temporary keymap, and
//! clipboard round trips. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::scenario::{
    Action, Position, Primitive, ScrollGlide, ScrollSource, TypingCadence, Wait, expand,
};
use fakeland::{
    Fakeland, InputConfig, LaunchSpec, OutputConfig, PointerDevice, Session, UnmappedKeys,
    WheelKind, WindowRef,
};
use fakeland_types::{Dur, Hz, Nanos, WindowId};
use serde_json::Value;

fn session(input: InputConfig) -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .input(input)
        .app_marks(true)
        .start()
        .expect("start")
}

fn launch(session: &Session, scene: &str, app_id: &str, log: &std::path::Path) -> WindowId {
    let app = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            scene,
            "--app-id",
            app_id,
            "--exit-after",
            "60",
            "--log-json",
            log.to_str().unwrap(),
        ]))
        .expect("launch");
    let w = session
        .blocking_wait_window(WindowRef::app_id(app_id), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    session
        .blocking_wait_settled(
            WindowRef::id(w.id),
            Nanos::from_millis(150),
            Nanos::from_secs(5),
        )
        .expect("settled");
    w.id
}

fn last_state(log: &[Value], key: &str) -> Value {
    support::log_events(log, "commit")
        .iter()
        .rev()
        .find_map(|c| c["state"].get(key).cloned())
        .unwrap_or(Value::Null)
}

fn settle(session: &Session, w: WindowId) {
    session
        .blocking_wait(Wait::settled(Some(WindowRef::id(w)), Dur::from_millis(150)))
        .expect("settle");
}

fn expected_scroll(action: &Action) -> f64 {
    expand(action)
        .unwrap()
        .unwrap()
        .primitives()
        .map(|p| match p {
            Primitive::PointerAxis { dy, .. } => *dy,
            _ => 0.0,
        })
        .sum()
}

fn scroll_case(input: InputConfig, source: ScrollSource, name: &str) -> (f64, f64, Vec<Value>) {
    let log_file = support::log_path(name);
    let s = session(input);
    let w = launch(&s, "scroll", name, &log_file);
    let action: Action = ScrollGlide::constant(2400.0, Dur::from_millis(1000))
        .at(Position::center(WindowRef::id(w)))
        .source(source)
        .into();
    let expected = expected_scroll(&action);
    let outcome = s.blocking_act(action).expect("scroll");
    settle(&s, w);
    let log = support::read_log(&log_file);
    let got = last_state(&log, "scroll")[1].as_f64().unwrap_or(f64::NAN);
    if let fakeland::ActionOutcome::Input { sent } = outcome {
        eprintln!("{name}: {} events sent", sent.len());
    }
    (expected, got, log)
}

#[test]
#[ignore = "runs the reference client"]
fn scroll_glides_move_the_content_by_the_curve_integral() {
    let _serial = support::serial();
    let (expected, got, log) = scroll_case(
        InputConfig::default(),
        ScrollSource::wheel(true),
        "scroll-hires",
    );
    eprintln!("hires wheel: expected {expected} px, client offset {got} px");
    assert!((expected - 2400.0).abs() < 1e-6);
    assert!((got - expected).abs() < 1.0, "{got} vs {expected}");
    let axis_events: Vec<&Value> = support::log_events(&log, "input_received")
        .into_iter()
        .filter(|l| l["kind"] == "pointer_axis")
        .collect();
    assert!(axis_events.iter().all(|l| l["marks_id"].as_u64().is_some()));
    let v120: i64 = axis_events
        .iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
        .filter_map(|e| e["value120_y"].as_i64())
        .sum();
    assert_eq!(v120, 19_200, "hires steps add up to 160 detents");

    let (expected, got, log) = scroll_case(
        InputConfig::default(),
        ScrollSource::finger(true),
        "scroll-finger",
    );
    eprintln!("finger: expected {expected} px, client offset {got} px");
    assert!((got - expected).abs() < 1.0);
    let stops = support::log_events(&log, "input_received")
        .into_iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
        .filter(|e| e["stop_y"] == true)
        .count();
    assert_eq!(stops, 1, "a finger scroll ends with one axis stop");
}

#[test]
#[ignore = "runs the reference client"]
fn a_classic_wheel_sends_whole_detents() {
    let _serial = support::serial();
    let input = InputConfig {
        pointer: PointerDevice {
            report_rate: None,
            wheel: WheelKind::Discrete,
        },
        ..InputConfig::default()
    };
    let (expected, got, log) = scroll_case(input, ScrollSource::wheel(true), "scroll-discrete");
    eprintln!("discrete wheel: expected {expected} px, client offset {got} px");
    assert!((got - 2400.0).abs() < 1.0);
    for e in support::log_events(&log, "input_received")
        .into_iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
    {
        if let Some(v) = e["value120_y"].as_i64() {
            assert_eq!(v % 120, 0, "only whole detents: {v}");
        }
    }
}

/// Rebuilds the text that the type scene shows from the client input log.
fn typed_text(log: &[Value]) -> String {
    let mut text = String::new();
    for l in support::log_events(log, "input_received") {
        for e in l["events"].as_array().cloned().unwrap_or_default() {
            match e["type"].as_str() {
                Some("key") if e["pressed"] == true => match e["name"].as_str() {
                    Some("Return") => text.push('\n'),
                    Some("BackSpace") => {
                        text.pop();
                    }
                    _ => text.push_str(e["utf8"].as_str().unwrap_or("")),
                },
                Some("text") => text.push_str(e["text"].as_str().unwrap_or("")),
                _ => {}
            }
        }
    }
    text
}

fn type_case(unmapped: UnmappedKeys, name: &str, text: &str) -> (String, Vec<Value>) {
    let log_file = support::log_path(name);
    let s = session(InputConfig {
        unmapped,
        ..InputConfig::default()
    });
    let w = launch(&s, "type", name, &log_file);
    s.blocking_click(Position::center(WindowRef::id(w)), Default::default())
        .expect("focus click");
    s.blocking_type_text(
        text,
        TypingCadence::default().with_hold(Dur::from_millis(10)),
    )
    .expect("type");
    settle(&s, w);
    let log = support::read_log(&log_file);
    (typed_text(&log), log)
}

#[test]
#[ignore = "runs the reference client"]
fn typing_produces_the_text() {
    let _serial = support::serial();
    let text = "Hello, World! 1+2=3 ~ äé€";
    let (got, log) = type_case(UnmappedKeys::TextInput, "type-text-input", text);
    assert_eq!(got, text);
    let commits = support::log_events(&log, "input_received")
        .into_iter()
        .filter(|l| l["kind"] == "text")
        .count();
    assert_eq!(commits, 2, "ä and é go through text input");
    let len = last_state(&log, "text_len").as_u64().unwrap_or(0);
    assert_eq!(len as usize, text.chars().count());

    let (got, log) = type_case(UnmappedKeys::Keymap, "type-keymap", text);
    assert_eq!(got, text, "a temporary keymap carries the missing keysyms");
    assert_eq!(
        support::log_events(&log, "input_received")
            .into_iter()
            .filter(|l| l["kind"] == "text")
            .count(),
        0
    );
}

#[test]
#[ignore = "runs the reference client"]
fn combos_press_modifiers_in_order() {
    let _serial = support::serial();
    let log_file = support::log_path("combo");
    let s = session(InputConfig::default());
    let w = launch(&s, "type", "combo", &log_file);
    s.blocking_click(Position::center(WindowRef::id(w)), Default::default())
        .expect("focus");
    s.blocking_combo("shift+a").expect("combo");
    s.blocking_key("BackSpace").expect("key");
    s.blocking_combo("ctrl+shift+b").expect("combo");
    settle(&s, w);
    let log = support::read_log(&log_file);
    let keys: Vec<(String, bool)> = support::log_events(&log, "input_received")
        .into_iter()
        .flat_map(|l| l["events"].as_array().cloned().unwrap_or_default())
        .filter(|e| e["type"] == "key")
        .map(|e| {
            (
                e["name"].as_str().unwrap_or("").to_string(),
                e["pressed"] == true,
            )
        })
        .collect();
    let want: Vec<(String, bool)> = [
        ("Shift_L", true),
        ("A", true),
        ("A", false),
        ("Shift_L", false),
        ("BackSpace", true),
        ("BackSpace", false),
        ("Control_L", true),
        ("Shift_L", true),
        ("B", true),
        ("B", false),
        ("Shift_L", false),
        ("Control_L", false),
    ]
    .iter()
    .map(|(n, p)| (n.to_string(), *p))
    .collect();
    assert_eq!(keys, want);
}

#[test]
#[ignore = "runs wl-copy and wl-paste"]
fn clipboard_round_trips() {
    let _serial = support::serial();
    if !support::have("wl-copy") || !support::have("wl-paste") {
        eprintln!("wl-clipboard is missing");
        return;
    }
    let s = session(InputConfig::default());
    s.blocking_act(Action::Clipboard(
        fakeland::scenario::ClipboardAction::text("from a scenario"),
    ))
    .expect("set");
    assert_eq!(
        s.blocking_clipboard_get(Nanos::from_secs(2)).expect("own"),
        "from a scenario"
    );
    let paste = s
        .blocking_launch(LaunchSpec::new("wl-paste").args(["--no-newline"]))
        .expect("wl-paste");
    let exit = paste.blocking_wait();
    assert_eq!(exit.code, Some(0), "{}", paste.stderr());
    assert_eq!(paste.stdout(), "from a scenario");

    let copy = s
        .blocking_launch(LaunchSpec::new("wl-copy").args(["--foreground", "from a client"]))
        .expect("wl-copy");
    let mut text = String::new();
    for _ in 0..50 {
        if let Ok(t) = s.blocking_clipboard_get(Nanos::from_millis(500))
            && t == "from a client"
        {
            text = t;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(text, "from a client", "{}", copy.stderr());
    let _ = copy.kill();
}
