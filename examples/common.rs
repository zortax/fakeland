//! Shared helpers of the examples: the path of the reference client, the app command after `--`,
//! flags, logging and the output directory.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Returns the path of the reference client: `FAKELAND_TESTCLIENT`, or the debug build in the
/// workspace target directory. Build it with `cargo build -p fakeland-testclient`.
pub fn testclient() -> String {
    if let Ok(path) = std::env::var("FAKELAND_TESTCLIENT") {
        return path;
    }
    let path = workspace().join("target/debug/fakeland-testclient");
    if !path.exists() {
        eprintln!(
            "{} is missing; run `cargo build -p fakeland-testclient` first",
            path.display()
        );
    }
    path.to_string_lossy().into_owned()
}

/// Returns the arguments after `--`, or `None` when there are none.
pub fn app_command() -> Option<Vec<String>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let i = args.iter().position(|a| a == "--")?;
    let rest = args[i + 1..].to_vec();
    (!rest.is_empty()).then_some(rest)
}

/// Returns true when a flag is in the arguments before `--`.
pub fn flag(name: &str) -> bool {
    std::env::args()
        .skip(1)
        .take_while(|a| a != "--")
        .any(|a| a == name)
}

/// Returns the value after an option before `--`, such as `--hz 144`.
pub fn value(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().skip(1).take_while(|a| a != "--").collect();
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

/// Returns the reference client command with `args`, as an argv vector.
pub fn testclient_argv(args: &[&str]) -> Vec<String> {
    std::iter::once(testclient())
        .chain(args.iter().map(|a| (*a).to_owned()))
        .collect()
}

/// Logs through `RUST_LOG`.
pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
}

/// Returns the directory for example output files, `target/examples`, and creates it.
pub fn out_dir() -> PathBuf {
    let dir = workspace().join("target/examples");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
    }
    dir
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}
