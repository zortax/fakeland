//! Wayland client for layer-shell and session-lock tests: it shows one layer surface or one
//! toplevel with an optional popup that may grab the seat, or locks the session with one lock
//! surface per output. Every configure and input event goes to stdout as a JSON line.

#![forbid(unsafe_code)]

mod app;
mod args;
mod input;
mod layer;
mod lock;
mod log;
mod noop;
mod shm;
mod xdg;

use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use serde_json::json;

use crate::app::App;
use crate::args::{Args, Mode};

fn main() -> ExitCode {
    let args = Args::parse();
    let exit_after = Duration::from_secs(args.exit_after);
    let (mut app, mut queue, _conn) = match App::connect(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("fakeland-layerclient: {e}");
            return ExitCode::from(1);
        }
    };
    std::thread::spawn(move || {
        std::thread::sleep(exit_after);
        log::event("timeout", json!({}));
        std::process::exit(0);
    });
    let qh = queue.handle();
    let started = match app.args.mode {
        Mode::Layer => app.create_layer(&qh),
        Mode::Lock => app.create_lock(&qh),
        Mode::Toplevel => app.create_toplevel(&qh),
    };
    if !started {
        eprintln!("fakeland-layerclient: the compositor lacks the protocol");
        return ExitCode::from(2);
    }
    while !app.done {
        if let Err(e) = queue.blocking_dispatch(&mut app) {
            eprintln!("fakeland-layerclient: {e}");
            return ExitCode::from(1);
        }
    }
    let _ = queue.roundtrip(&mut app);
    ExitCode::SUCCESS
}
