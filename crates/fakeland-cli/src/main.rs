//! The fakeland command: run and bench apps, serve a control socket, drive a running session, and
//! export, report and compare traces.

#![forbid(unsafe_code)]

mod bench_args;
mod cli;
mod commands;
mod ctl;
mod session_args;
mod util;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("FAKELAND_LOG").unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("warn,smithay::backend::drm=error")
            }),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let code = match execute(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fakeland: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn execute(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Ctl(args) => ctl::run(args),
        Command::Export(args) => commands::export::run(args),
        Command::Report(args) => commands::report::report(args),
        Command::Compare(args) => commands::report::compare_reports(args),
        Command::Presets(args) => commands::report::presets(args),
        Command::Bench(args) => runtime()?.block_on(commands::bench::run(*args)),
        Command::Run(args) => runtime()?.block_on(commands::run::run(*args)),
        Command::Serve(args) => runtime()?.block_on(commands::serve::run(*args)),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?)
}
