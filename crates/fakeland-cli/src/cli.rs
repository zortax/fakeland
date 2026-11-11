//! The command line of `fakeland`.

use clap::{Parser, Subcommand};

use crate::commands::bench::BenchArgs;
use crate::commands::report::{CompareArgs, PresetsArgs, ReportArgs};
use crate::commands::{export::ExportArgs, run::RunArgs, serve::ServeArgs};
use crate::ctl::CtlArgs;

/// Headless Wayland compositor for benchmarks and scripted app control.
#[derive(Debug, Parser)]
#[command(name = "fakeland", version, about)]
pub struct Cli {
    /// The command.
    #[command(subcommand)]
    pub command: Command,
}

/// A `fakeland` command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run an app headless, write its trace and print a frame summary.
    Run(Box<RunArgs>),
    /// Run a bench preset or a scenario against an app and print the report.
    Bench(Box<BenchArgs>),
    /// Run a session with a control socket until a signal or `fakeland ctl stop`.
    Serve(Box<ServeArgs>),
    /// Drive a running session through its control socket.
    Ctl(CtlArgs),
    /// Convert a trace file and print its summary.
    Export(ExportArgs),
    /// Print a bench report.
    Report(ReportArgs),
    /// Compare a bench report with a baseline. Exits with 3 on a regression.
    Compare(CompareArgs),
    /// List the bench presets, or show the parameters of one.
    Presets(PresetsArgs),
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_typical_command_lines() {
        let cli = Cli::try_parse_from([
            "fakeland",
            "run",
            "--output",
            "2560x1440@143.912",
            "--scale",
            "1.5",
            "--timing",
            "vrr:48-144",
            "--latch",
            "1.5ms",
            "--callbacks",
            "before-vblank:3ms",
            "--duration",
            "10s",
            "--screenshot-at",
            "2s:out.png",
            "--",
            "vkcube",
            "--wsi",
            "wayland",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Run(_)));
        let cli = Cli::try_parse_from([
            "fakeland",
            "bench",
            "hover-latency",
            "--param",
            "iterations=5",
            "--report",
            "r.json",
            "--output",
            "1920x1080@144",
            "--",
            "alacritty",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Bench(_)));
        let cli = Cli::try_parse_from(["fakeland", "ctl", "scroll", "10", "20", "-120"]).unwrap();
        assert!(matches!(cli.command, Command::Ctl(_)));
        let cli =
            Cli::try_parse_from(["fakeland", "ctl", "--json", "click", "1", "2", "right"]).unwrap();
        assert!(matches!(cli.command, Command::Ctl(_)));
    }
}
