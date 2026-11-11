//! Runs presets against the reference client and prints their reports. Run with
//! `cargo bench -p fakeland-bench` after `cargo build -p fakeland-testclient`; pass names to
//! filter, such as `cargo bench -p fakeland-bench -- hover`. Reports land in
//! `target/fakeland-bench`; `FAKELAND_BENCH_SAVE_BASELINE=1` saves them as the baseline that
//! later runs compare against.

use std::path::PathBuf;
use std::process::ExitCode;

use fakeland::{Fakeland, LaunchSpec, OutputConfig, Session, WindowRef};
use fakeland_bench::Harness;
use fakeland_bench::presets::{PresetParams, by_name};
use fakeland_types::{Hz, Nanos};
use serde_json::json;

/// One preset run: the reference client arguments and the preset parameters.
struct Case {
    name: &'static str,
    preset: &'static str,
    client: &'static [&'static str],
    params: fn() -> PresetParams,
}

const CASES: &[Case] = &[
    Case {
        name: "pacing-jank",
        preset: "pacing",
        client: &[
            "--scene",
            "animate",
            "--mode",
            "callback",
            "--render-delay",
            "7ms",
            "--jank-every",
            "10",
            "--jank",
            "25ms",
        ],
        params: || PresetParams::new().set("duration", "3s"),
    },
    Case {
        name: "hover-latency",
        preset: "hover-latency",
        client: &[
            "--scene",
            "hover",
            "--mode",
            "on-demand",
            "--render-delay",
            "7ms",
        ],
        params: || {
            PresetParams::new()
                .set("position", json!({"x": 200, "y": 175}))
                .set("away", json!({"x": 20, "y": 20}))
                .set("roi", json!({"x": 100, "y": 100, "w": 200, "h": 150}))
                .set("repeats", 16)
        },
    },
    Case {
        name: "scroll-glide",
        preset: "scroll-glide",
        client: &["--scene", "scroll", "--mode", "callback"],
        params: || {
            PresetParams::new()
                .set("roi", json!({"x": 440, "y": 160, "w": 400, "h": 400}))
                .set("velocity", 1800.0)
        },
    },
    Case {
        name: "typing",
        preset: "typing",
        client: &["--scene", "type", "--mode", "on-demand"],
        params: || PresetParams::new().set("wpm", 160),
    },
    Case {
        name: "resize-storm",
        preset: "resize-storm",
        client: &["--scene", "resize", "--mode", "on-demand"],
        params: PresetParams::new,
    },
    Case {
        name: "idle",
        preset: "idle",
        client: &["--scene", "idle"],
        params: || PresetParams::new().set("duration", "2s"),
    },
    Case {
        name: "throughput",
        preset: "throughput",
        client: &[
            "--scene",
            "animate",
            "--mode",
            "unthrottled",
            "--buffers",
            "4",
        ],
        params: || PresetParams::new().set("duration", "2s"),
    },
];

fn testclient() -> PathBuf {
    std::env::var_os("FAKELAND_TESTCLIENT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/fakeland-testclient")
        })
}

fn session() -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()
        .expect("start a session")
}

fn main() -> ExitCode {
    let mut harness = Harness::from_env().default_out_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fakeland-bench"),
    );
    let client = testclient();
    if !client.exists() {
        eprintln!("build the reference client first: cargo build -p fakeland-testclient");
        return ExitCode::FAILURE;
    }
    for case in CASES {
        if !harness.enabled(case.name) {
            continue;
        }
        let s = session();
        let app_id = format!("bench.{}", case.name);
        let mut args = vec!["--app-id", app_id.as_str(), "--exit-after", "300"];
        args.extend_from_slice(case.client);
        let app = s
            .blocking_launch(LaunchSpec::new(client.to_string_lossy()).args(args))
            .expect("launch the reference client");
        if s.blocking_wait_window(WindowRef::app_id(&app_id), Nanos::from_secs(10))
            .is_err()
        {
            eprintln!("{}: no window: {}", case.name, app.stderr());
            continue;
        }
        let params = (case.params)().set("target", json!({"type": "app_id", "app_id": app_id}));
        let bench = by_name(case.preset)
            .expect("preset")
            .bench(&params)
            .expect("preset parameters");
        harness.run(case.name, &bench, &s);
        let _ = s.blocking_shutdown();
    }
    harness.finish()
}
