//! End-to-end test of the accessibility tools over stdio with a GTK dialog: the tree, a search,
//! a text change and a click that ends the dialog.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

fn have(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
    next: u64,
}

impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fakeland-mcp"))
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn fakeland-mcp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            lines,
            next: 1,
        }
    }

    fn send(&mut self, msg: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("no answer to {method}"));
            let v: Value = serde_json::from_str(&line).expect("json line");
            if v.get("id") == Some(&json!(id)) {
                return v;
            }
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        let v = self.request("tools/call", json!({"name": tool, "arguments": args}));
        let result = v
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("{tool}: {v}"));
        eprintln!("{tool}: {}", texts(&result).join(" | "));
        result
    }
}

fn texts(result: &Value) -> Vec<String> {
    result["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["text"].as_str().map(str::to_owned))
        .collect()
}

fn is_error(result: &Value) -> bool {
    result["isError"].as_bool().unwrap_or(false)
}

/// Returns the id after `#` on the first line that contains `needle`.
fn id_on_line(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in\n{text}"));
    line.split('#')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("id")
        .to_string()
}

#[test]
#[ignore = "starts a session; run with `just it`"]
fn reads_and_drives_a_gtk_dialog() {
    if !have("zenity") {
        eprintln!("zenity is missing");
        return;
    }
    let mut c = Client::start();
    c.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "fakeland-test", "version": "0"}
        }),
    );
    c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let r = c.call(
        "session_start",
        json!({"outputs": ["1280x720@60"], "renderer": "pixman"}),
    );
    assert!(!is_error(&r), "{r}");
    let r = c.call(
        "launch",
        json!({"command": "zenity", "args": ["--entry", "--text", "Your name"]}),
    );
    assert!(!is_error(&r), "{r}");

    let mut tree = String::new();
    for _ in 0..50 {
        let r = c.call("a11y_tree", json!({}));
        assert!(!is_error(&r), "{r}");
        tree = texts(&r)[0].clone();
        if tree.contains("button \"OK\"") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(tree.contains("dialog"), "{tree}");
    assert!(tree.contains("button \"OK\""), "{tree}");
    assert!(tree.contains(" @"), "bounds: {tree}");

    let r = c.call(
        "a11y_find",
        json!({"states": ["editable"], "role": "entry"}),
    );
    assert!(!is_error(&r), "{r}");
    let entry = id_on_line(&texts(&r)[0], "editable");
    let r = c.call("a11y_set_text", json!({"node": entry, "text": "from mcp"}));
    assert!(!is_error(&r), "{r}");

    let r = c.call("a11y_find", json!({"role": "button", "name": "^nothing$"}));
    assert_eq!(texts(&r)[0], "No element matches.");
    let r = c.call("a11y_action", json!({"node": "n424242"}));
    assert!(is_error(&r));

    let ok = id_on_line(&tree, "button \"OK\"");
    let r = c.call("a11y_action", json!({"node": ok, "via": "pointer"}));
    assert!(!is_error(&r), "{r}");
    assert!(texts(&r)[0].contains("with the pointer"), "{r}");

    let mut out = String::new();
    for _ in 0..50 {
        let r = c.call("logs", json!({}));
        out = texts(&r)[0].clone();
        if out.contains("from mcp") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(out.contains("from mcp"), "{out}");

    let r = c.call("session_stop", json!({}));
    assert!(!is_error(&r));
    drop(c.stdin.take());
    for _ in 0..100 {
        if c.child.try_wait().unwrap().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = c.child.kill();
    panic!("fakeland-mcp did not exit after stdin closed");
}
