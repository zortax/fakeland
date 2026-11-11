//! `fakeland serve`: run a session with a control socket until a signal or `fakeland ctl stop`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use fakeland::LaunchSpec;
use fakeland_control::methods::LaunchParams;
use fakeland_control::{Controller, serve};
use fakeland_types::Dur;

use crate::session_args::SessionArgs;
use crate::util::{absolute, parse_secs, signal};

/// Flags of `fakeland serve`.
#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(flatten)]
    session: SessionArgs,
    /// The control socket. The default is `$XDG_RUNTIME_DIR/fakeland/<wayland socket>.sock`.
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
    /// Stop when the launched app exits.
    #[arg(long)]
    exit_with_app: bool,
    /// Wait this long for the first window of the launched app before printing the ready line.
    #[arg(long, value_name = "DUR", value_parser = parse_secs)]
    wait_window: Option<Dur>,
    /// A command to launch, after `--`.
    #[arg(last = true, value_name = "CMD")]
    command: Vec<String>,
}

/// Runs `fakeland serve`.
pub async fn run(args: ServeArgs) -> Result<i32> {
    let socket = args.socket.as_deref().map(absolute).transpose()?;
    let session_args = args.session.clone();
    let session = tokio::task::spawn_blocking(move || session_args.start()).await??;
    let controller = Controller::new(session.clone());
    let mut server = serve(controller.clone(), socket.as_deref()).context("cannot serve")?;
    let app = match LaunchSpec::from_argv(args.command.clone()) {
        Some(spec) => {
            let info = controller
                .launch(LaunchParams {
                    spec,
                    wait_window: args.wait_window,
                })
                .await?;
            eprintln!("fakeland: {} runs as pid {}", info.command, info.pid.0);
            Some(controller.app(Some(info.pid))?)
        }
        None => None,
    };
    println!("WAYLAND_DISPLAY={}", session.socket_name());
    println!("FAKELAND_SOCKET={}", server.path().display());
    eprintln!("fakeland: serving; stop with Ctrl-C or `fakeland ctl stop`");
    let stop = server.stop_handle();
    let stopped = tokio::task::spawn_blocking(move || stop.wait());
    let reason = tokio::select! {
        _ = signal() => "signal",
        _ = stopped => "stop request",
        _ = async {
            match (&app, args.exit_with_app) {
                (Some(app), true) => { app.wait().await; }
                _ => std::future::pending::<()>().await,
            }
        } => "app exit",
    };
    eprintln!("fakeland: stopping after {reason}");
    server.stop_handle().stop();
    server.stop();
    tokio::task::spawn_blocking(move || session.blocking_shutdown()).await??;
    Ok(0)
}
