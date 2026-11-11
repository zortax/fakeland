//! End-to-end test of the MCP server: the binary speaks JSON-RPC over stdio while the test drives
//! the reference client through its tools.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

fn testclient() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/fakeland-testclient");
    assert!(
        path.exists(),
        "build the reference client first: cargo build -p fakeland-testclient"
    );
    path.to_string_lossy().into_owned()
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
        let stdout: ChildStdout = child.stdout.take().unwrap();
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

fn image(result: &Value) -> fakeland::Image {
    let block = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image block");
    assert_eq!(block["mimeType"], "image/png");
    let png = base64::engine::general_purpose::STANDARD
        .decode(block["data"].as_str().unwrap())
        .unwrap();
    fakeland::Image::decode_png(&png).unwrap()
}

fn is_error(result: &Value) -> bool {
    result["isError"].as_bool().unwrap_or(false)
}

#[test]
#[ignore = "starts a session; run with `just it`"]
fn drives_the_reference_client_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = Client::start();

    let init = c.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "fakeland-test", "version": "0"}
        }),
    );
    let result = &init["result"];
    assert_eq!(result["serverInfo"]["name"], "fakeland");
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("screenshot")
    );
    assert!(result["capabilities"]["tools"].is_object());
    c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let list = c.request("tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for want in [
        "session_start",
        "session_stop",
        "launch",
        "list_windows",
        "focus_window",
        "screenshot",
        "click",
        "double_click",
        "right_click",
        "move_pointer",
        "hover",
        "drag",
        "scroll",
        "type_text",
        "key",
        "wait_for",
        "logs",
        "set_output",
        "clipboard_get",
        "clipboard_set",
        "run_scenario",
        "scenario_schema",
        "measure_latency",
        "frame_stats",
        "trace_start",
        "trace_stop",
        "kill_app",
        "restart_app",
        "run_benchmark",
        "a11y_tree",
        "a11y_find",
        "a11y_action",
        "a11y_set_text",
    ] {
        assert!(names.contains(&want), "missing tool {want}: {names:?}");
    }
    for tool in list["result"]["tools"].as_array().unwrap() {
        assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()));
        assert_eq!(tool["inputSchema"]["type"], "object");
    }

    let r = c.call(
        "session_start",
        json!({"outputs": ["1280x720@60"], "renderer": "pixman"}),
    );
    assert!(!is_error(&r));
    assert!(texts(&r)[0].contains("1280x720@60.000Hz"));

    let r = c.call(
        "launch",
        json!({"command": testclient(), "args": ["--scene", "hover,click,type", "--mode", "on-demand"]}),
    );
    assert!(!is_error(&r), "{r}");
    assert!(texts(&r)[0].contains("fakeland-testclient"));

    // The first screenshot sets the pixel frame: 640 px wide over 1280 logical px.
    let r = c.call("screenshot", json!({"max_dim": 640}));
    let img = image(&r);
    assert_eq!((img.width(), img.height()), (640, 360));
    assert!(texts(&r)[0].contains("1 px = 2.0000 logical px"));

    // The hover rect spans logical 100..300 x 100..250, so pixel (100, 87) lies inside.
    let r = c.call("hover", json!({"x": 100, "y": 87, "screenshot": true}));
    assert!(texts(&r)[0].contains("logical (200.0, 174.0)"));
    let img = image(&r);
    assert_eq!(
        img.pixel(100, 87).map(|p| [p[0], p[1], p[2]]),
        Some([0xe0, 0x60, 0x20])
    );

    let r = c.call("click", json!({"x": 400, "y": 300, "screenshot": true}));
    assert!(!is_error(&r));
    let img = image(&r);
    assert_eq!(
        img.pixel(500, 300).map(|p| [p[0], p[1], p[2]]),
        Some([0xc0, 0xc0, 0x30])
    );

    let r = c.call("type_text", json!({"text": "abc"}));
    assert!(texts(&r)[0].starts_with("Typed 3 characters"));
    let r = c.call("key", json!({"combo": "Return"}));
    assert!(!is_error(&r));
    let r = c.call("wait_for", json!({"until": "settled", "quiet_ms": 100}));
    assert!(!is_error(&r), "{r}");

    let r = c.call("click", json!({"x": 5000, "y": 10}));
    assert!(is_error(&r));
    assert!(texts(&r)[0].contains("outside the last screenshot"));

    let r = c.call("trace_start", json!({}));
    assert!(!is_error(&r));
    let r = c.call(
        "measure_latency",
        json!({"action": "hover", "x": 150, "y": 150, "away": [600, 500], "logical": true,
               "roi": {"x": 100, "y": 100, "w": 200, "h": 150}, "repeat": 4}),
    );
    assert!(!is_error(&r), "{r}");
    let report: Value = serde_json::from_str(&texts(&r)[1]).unwrap();
    let detected = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["stages"]["total"].is_number())
        .count();
    assert!(detected >= 3, "{report}");
    let trace = tmp.path().join("mcp.perfetto.json");
    let r = c.call("trace_stop", json!({"path": trace.to_str().unwrap()}));
    assert!(!is_error(&r));
    assert!(trace.exists());

    let r = c.call("frame_stats", json!({"window": "any", "last_ms": 10000}));
    assert!(texts(&r)[0].contains("presented"));

    let r = c.call(
        "run_scenario",
        json!({"scenario": {"name": "s", "steps": [
            {"action": {"type": "pointer", "op": "move_to", "pos": {"type": "window", "x": 150, "y": 150}}},
            {"anchor": {"type": "next_vblank"}, "action": {"type": "mark", "label": "x"}}
        ]}}),
    );
    assert!(!is_error(&r), "{r}");
    assert!(texts(&r)[0].contains(": ok in"));

    let r = c.call("run_benchmark", json!({"preset": "list"}));
    assert!(texts(&r)[0].contains("hover-latency"));
    let r = c.call(
        "run_benchmark",
        json!({"preset": "pacing", "params": {"duration": "1s", "warmup": 0}, "iterations": 1, "window": "any"}),
    );
    assert!(!is_error(&r), "{r}");
    assert!(texts(&r)[0].contains("frame_pacing"));
    let r = c.call("run_benchmark", json!({"preset": "no-such-preset"}));
    assert!(is_error(&r));

    let r = c.call("clipboard_set", json!({"text": "hello"}));
    assert!(!is_error(&r));
    let r = c.call("clipboard_get", json!({}));
    assert_eq!(texts(&r)[0], "hello");

    let r = c.call("logs", json!({}));
    assert!(texts(&r)[0].contains("running"));
    let r = c.call("list_windows", json!({}));
    assert!(texts(&r)[0].contains("fakeland-testclient"));
    let r = c.call("kill_app", json!({}));
    assert!(texts(&r)[0].contains("signal 15"));

    let bad = c.request(
        "tools/call",
        json!({"name": "no_such_tool", "arguments": {}}),
    );
    assert!(bad.get("error").is_some() || is_error(&bad["result"]));

    let r = c.call("session_stop", json!({}));
    assert!(texts(&r)[0].contains("stopped"));

    drop(c.stdin.take());
    for _ in 0..100 {
        if let Some(status) = c.child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = c.child.kill();
    panic!("fakeland-mcp did not exit after stdin closed");
}
