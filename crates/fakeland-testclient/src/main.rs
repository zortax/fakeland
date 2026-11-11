//! Reference Wayland client with controllable behavior: present modes, scenes for hover, click,
//! scroll, typing, animation and resize, injected render delay, jank and input delay. It reports
//! ground truth through fakeland_marks_v1 app marks and a JSON lines event log.

#![forbid(unsafe_code)]

mod app;
mod cli;
mod draw;
mod input;
mod load;
mod log;
mod pace;
mod scene;
mod wl;

use std::process::ExitCode;

use clap::Parser;
use fakeland_types::{Clock, MonotonicClock};

use crate::app::{App, ExitReason, USAGE_EXIT_CODE, run};
use crate::cli::Args;
use crate::load::{DelayKind, wait_until};
use crate::log::{EventLog, LogEvent};

fn main() -> ExitCode {
    let clock = MonotonicClock;
    let started = clock.now();
    let args = Args::parse();
    if let Err(e) = args.validate() {
        eprintln!("fakeland-testclient: {e}");
        return ExitCode::from(USAGE_EXIT_CODE as u8);
    }
    let mut log = match &args.log_json {
        Some(path) => match EventLog::open(path) {
            Ok(log) => log,
            Err(e) => {
                eprintln!("fakeland-testclient: cannot open {}: {e}", path.display());
                return ExitCode::from(1);
            }
        },
        None => EventLog::disabled(),
    };
    log.write(
        started,
        &LogEvent::Start {
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            args: std::env::args().collect(),
        },
    );
    if args.startup_delay.0 > 0 {
        wait_until(&clock, started + args.startup_delay, DelayKind::Sleep);
    }
    let reason = match App::connect(args, &mut log, started) {
        Ok((mut app, mut queue)) => {
            let reason = run(&mut app, &mut queue);
            if matches!(
                reason,
                ExitReason::Closed | ExitReason::Timeout | ExitReason::FramesDone
            ) {
                app.shutdown();
            }
            finish(&mut app.log, &reason);
            reason
        }
        Err(reason) => {
            finish(&mut log, &reason);
            reason
        }
    };
    ExitCode::from(reason.code() as u8)
}

/// Logs the exit and reports a failure on stderr.
fn finish(log: &mut EventLog, reason: &ExitReason) {
    log.write(
        MonotonicClock.now(),
        &LogEvent::Exit {
            code: reason.code(),
            reason: reason.to_string(),
        },
    );
    if reason.code() != 0 {
        eprintln!("fakeland-testclient: {reason}");
    }
}
