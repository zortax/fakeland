//! End-to-end test of the resource and recording tools over stdio: a session with sampling, the
//! reference client animating, a Y4M recording of its window and the resources tool.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

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
    _child: Child,
    stdin: ChildStdin,
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
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut c = Self {
            _child: child,
            stdin,
            lines,
            next: 1,
        };
        c.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
        );
        c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }

    fn send(&mut self, msg: &Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
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
        let result = v["result"].clone();
        let text: Vec<&str> = result["content"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c["text"].as_str()).collect())
            .unwrap_or_default();
        eprintln!("{tool}: {}", text.first().copied().unwrap_or(""));
        assert!(!result["isError"].as_bool().unwrap_or(false), "{tool}: {v}");
        result
    }
}

/// Returns the JSON detail block of a result.
fn detail(result: &Value) -> Value {
    let text = result["content"][1]["text"].as_str().expect("a JSON block");
    serde_json::from_str(text).unwrap()
}

#[test]
#[ignore = "starts a session; run with `just it`"]
fn records_video_and_reports_resources() {
    let mut c = Client::start();
    let names: Vec<String> = c.request("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for want in ["resources", "record_start", "record_stop"] {
        assert!(names.iter().any(|n| n == want), "missing {want}");
    }
    c.call(
        "session_start",
        json!({"outputs": ["1280x720@60"], "renderer": "pixman", "sample": "100ms"}),
    );
    c.call(
        "launch",
        json!({"command": testclient(), "args": ["--scene", "animate", "--cpu-burn", "4ms"]}),
    );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fakeland-mcp-media");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("animate.y4m");
    let started = c.call(
        "record_start",
        json!({"path": path.to_str().unwrap(), "window": "any", "max_dim": 640}),
    );
    let info = detail(&started);
    assert_eq!(
        (info["width"].as_u64(), info["height"].as_u64()),
        (Some(640), Some(360))
    );
    std::thread::sleep(Duration::from_millis(1500));
    let resources = c.call("resources", json!({}));
    let snap = detail(&resources);
    assert_eq!(snap["enabled"], true);
    let procs = snap["processes"].as_array().unwrap();
    assert_eq!(procs.len(), 1, "{snap}");
    let cpu = procs[0]["cpu_percent"].as_f64().unwrap();
    assert!(
        cpu > 5.0 && cpu < 150.0,
        "4 ms of work per 16.7 ms frame: {cpu}"
    );
    assert!(procs[0]["rss_bytes"].as_u64().unwrap() > 1 << 20);
    let stopped = c.call("record_stop", json!({}));
    let report = detail(&stopped);
    let frames = report["frames"].as_u64().unwrap();
    assert!((85..=100).contains(&frames), "{report}");
    assert!(
        report["rendered"].as_u64().unwrap() > frames * 9 / 10,
        "{report}"
    );
    assert!(report["error"].is_null());
    let bytes = std::fs::metadata(&path).unwrap().len();
    let rate = &report["rate"];
    let header = format!(
        "YUV4MPEG2 W640 H360 F{}:{} Ip A1:1 C420jpeg\n",
        rate["num"], rate["den"]
    )
    .len() as u64;
    assert_eq!(bytes, header + frames * (6 + 640 * 360 * 3 / 2));
    c.call("session_stop", json!({}));
}
