//! Small helpers of the commands: paths, durations and signals.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fakeland_types::Dur;

/// Returns an absolute form of a path, relative to the current directory.
pub fn absolute(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("cannot resolve {}", path.display()))
}

/// Parses a duration such as `10s`, `250ms` or `1.5`. A bare number is in milliseconds.
pub fn parse_dur(s: &str) -> Result<Dur, String> {
    s.parse::<Dur>().map_err(|e| format!("{s:?}: {e}"))
}

/// Parses a duration where a bare number is in seconds, such as `--duration 10`.
pub fn parse_secs(s: &str) -> Result<Dur, String> {
    match s.trim().parse::<f64>() {
        Ok(secs) if secs.is_finite() && secs >= 0.0 => Ok(Dur::from_secs_f64(secs)),
        _ => parse_dur(s),
    }
}

/// Waits for SIGINT or SIGTERM.
pub async fn signal() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = async {
            match term.as_mut() {
                Some(t) => { t.recv().await; }
                None => std::future::pending::<()>().await,
            }
        } => {}
    }
}

/// Prints a value as pretty JSON.
pub fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_secs("10").unwrap(), Dur::from_secs(10));
        assert_eq!(parse_secs("1.5").unwrap(), Dur::from_millis(1500));
        assert_eq!(parse_secs("250ms").unwrap(), Dur::from_millis(250));
        assert_eq!(parse_dur("250").unwrap(), Dur::from_millis(250));
        assert!(parse_dur("soon").is_err());
    }
}
