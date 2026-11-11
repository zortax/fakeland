//! Shared helpers of the integration tests: a lock that runs one session at a time, the output
//! directory for screenshots, the reference client path and image checks.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use fakeland::{Event, Image, Record, Trace};

static SESSION_LOCK: Mutex<()> = Mutex::new(());

/// Runs one session at a time, so timing checks see an idle machine.
pub fn serial() -> MutexGuard<'static, ()> {
    SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Returns the directory for screenshots and traces of the tests.
pub fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fakeland-it");
    std::fs::create_dir_all(&dir).expect("output dir");
    dir
}

/// Saves an image as PNG and returns its path.
pub fn save(image: &Image, name: &str) -> PathBuf {
    let path = out_dir().join(name);
    std::fs::write(&path, image.encode_png().expect("png")).expect("write png");
    eprintln!("saved {}", path.display());
    path
}

/// Returns the path of the reference client.
pub fn testclient() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/fakeland-testclient");
    assert!(
        path.exists(),
        "build the reference client first: cargo build -p fakeland-testclient"
    );
    path.to_string_lossy().into_owned()
}

/// Returns true when a program is on `PATH`.
pub fn have(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// Returns the count of distinct colors in a sample grid of the image.
pub fn distinct_colors(image: &Image) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    let (w, h) = (image.width(), image.height());
    for y in (0..h).step_by((h as usize / 64).max(1)) {
        for x in (0..w).step_by((w as usize / 64).max(1)) {
            if let Some(px) = image.pixel(x, y) {
                seen.insert([px[0], px[1], px[2]]);
            }
        }
    }
    seen.len()
}

/// Returns the records of a trace that match a predicate.
pub fn records<'a>(
    trace: &'a Trace,
    f: impl Fn(&Event) -> bool + 'a,
) -> impl Iterator<Item = &'a Record> + 'a {
    trace.records.iter().filter(move |r| f(&r.event))
}

/// Returns the count of records that match a predicate.
pub fn count(trace: &Trace, f: impl Fn(&Event) -> bool) -> usize {
    trace.records.iter().filter(|r| f(&r.event)).count()
}

/// Reads the JSON log of the reference client: one object per line.
pub fn read_log(path: &std::path::Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Returns the log lines of one event kind.
pub fn log_events<'a>(log: &'a [serde_json::Value], event: &str) -> Vec<&'a serde_json::Value> {
    log.iter().filter(|v| v["event"] == event).collect()
}

/// Returns a fresh path for a client log in the output directory.
pub fn log_path(name: &str) -> PathBuf {
    let path = out_dir().join(format!("{name}.jsonl"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Prints a summary of nanosecond samples in milliseconds and returns (min, median, max).
pub fn summarize(label: &str, values: &[i64]) -> (f64, f64, f64) {
    let mut v: Vec<f64> = values.iter().map(|n| *n as f64 / 1e6).collect();
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        eprintln!("{label}: no samples");
        return (0.0, 0.0, 0.0);
    }
    let (min, med, max) = (v[0], v[v.len() / 2], v[v.len() - 1]);
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    eprintln!(
        "{label}: n={} min={min:.4} median={med:.4} mean={mean:.4} max={max:.4} ms",
        v.len()
    );
    (min, med, max)
}
