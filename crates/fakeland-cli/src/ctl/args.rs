//! The flags and subcommands of `fakeland ctl`.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use fakeland_types::Dur;

use super::position::Frame;
use crate::bench_args::BenchSelect;
use crate::util::{parse_dur, parse_secs};

/// Flags of `fakeland ctl`.
#[derive(Debug, Args)]
pub struct CtlArgs {
    /// The control socket. The default is FAKELAND_SOCKET, or the only running socket.
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
    /// Print results as JSON.
    #[arg(long, global = true)]
    pub json: bool,
    /// The command.
    #[command(subcommand)]
    pub command: CtlCommand,
}

/// A `fakeland ctl` command.
#[derive(Debug, Subcommand)]
pub enum CtlCommand {
    /// Print facts about the session.
    Info,
    /// List windows.
    Windows,
    /// List outputs.
    Outputs,
    /// List launched apps.
    Apps,
    /// Launch an app.
    Launch {
        /// Wait this long for its first window.
        #[arg(long, value_name = "DUR", value_parser = parse_secs)]
        wait: Option<Dur>,
        /// An environment variable as KEY=VALUE.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// The command, after `--`.
        #[arg(required = true, last = true, value_name = "CMD")]
        command: Vec<String>,
    },
    /// Stop an app with SIGTERM, or SIGKILL with --force.
    Kill {
        /// The pid. The default is the newest app.
        pid: Option<u32>,
        /// Send SIGKILL.
        #[arg(long)]
        force: bool,
    },
    /// Print the last lines of stdout and stderr of an app.
    Logs {
        /// The pid. The default is the newest app.
        pid: Option<u32>,
        /// The count of lines of each stream.
        #[arg(long, default_value_t = 200)]
        lines: usize,
    },
    /// Write a screenshot as PNG.
    Shot {
        /// The PNG file.
        file: PathBuf,
        /// Capture this window only.
        #[arg(long, value_name = "WINDOW")]
        window: Option<String>,
        /// Capture this output id.
        #[arg(long, value_name = "ID")]
        output: Option<u32>,
        /// Capture a region `X,Y,W,H` of the output, or of the window with --window.
        #[arg(long, value_name = "X,Y,W,H")]
        region: Option<String>,
        /// Downscale so the longer side is at most this many pixels.
        #[arg(long, value_name = "PX")]
        max_dim: Option<u32>,
        /// Draw the cursor.
        #[arg(long)]
        cursor: bool,
    },
    /// Move the pointer.
    Move {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
        #[command(flatten)]
        frame: Frame,
    },
    /// Click a button: left, right, middle or an evdev code.
    Click {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
        /// The button.
        #[arg(default_value = "left")]
        button: String,
        /// The count of clicks.
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[command(flatten)]
        frame: Frame,
    },
    /// Double-click the left button.
    Dclick {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
        #[command(flatten)]
        frame: Frame,
    },
    /// Click the right button.
    Rclick {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
        #[command(flatten)]
        frame: Frame,
    },
    /// Scroll at a position. Positive distances scroll down or right.
    Scroll {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
        /// The vertical distance in logical pixels.
        #[arg(allow_negative_numbers = true)]
        dy: f64,
        /// The horizontal distance in logical pixels.
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        dx: f64,
        /// Spread the distance over this time.
        #[arg(long, value_name = "DUR", value_parser = parse_dur)]
        smooth: Option<Dur>,
        /// Scroll as two fingers on a touchpad.
        #[arg(long)]
        finger: bool,
        /// Scroll as a wheel with whole detents only.
        #[arg(long)]
        detents: bool,
        #[command(flatten)]
        frame: Frame,
    },
    /// Drag with the left button.
    Drag {
        /// Start x.
        x1: f64,
        /// Start y.
        y1: f64,
        /// End x.
        x2: f64,
        /// End y.
        y2: f64,
        /// The drag time.
        #[arg(long, value_name = "DUR", value_parser = parse_dur, default_value = "300ms")]
        duration: Dur,
        #[command(flatten)]
        frame: Frame,
    },
    /// Type text.
    Type {
        /// The text.
        text: String,
        /// Words per minute.
        #[arg(long, default_value_t = 300.0)]
        wpm: f64,
        /// Send the text as one text-input commit.
        #[arg(long)]
        commit: bool,
    },
    /// Press a key or a combination, such as `Return` or `ctrl+shift+t`.
    Key {
        /// The key or combination.
        combo: String,
    },
    /// Wait on the compositor for a window, settled content or frames.
    #[command(subcommand)]
    Wait(WaitCommand),
    /// Run a scenario JSON file.
    Run {
        /// The scenario file.
        file: PathBuf,
        /// Write the trace of the run.
        #[arg(long, value_name = "FILE")]
        trace: Option<PathBuf>,
        /// Write capture steps and frame captures as PNG files here.
        #[arg(long, value_name = "DIR")]
        captures: Option<PathBuf>,
        /// Print the run id and return at once.
        #[arg(long)]
        detach: bool,
    },
    /// Print the state of a run, after a wait for its end.
    RunResult {
        /// The run id.
        run: u64,
        /// Wait this long for the end.
        #[arg(long, value_name = "DUR", value_parser = parse_secs)]
        wait: Option<Dur>,
    },
    /// Run a probe spec JSON file and print its report.
    Probe {
        /// The probe spec file.
        file: PathBuf,
    },
    /// Install or remove trigger sets.
    #[command(subcommand)]
    Triggers(TriggersCommand),
    /// Capture and write traces.
    #[command(subcommand)]
    Trace(TraceCommand),
    /// Print frame statistics of a window over the recent past.
    Stats {
        /// The window.
        #[arg(long, value_name = "WINDOW", default_value = "focused")]
        window: String,
        /// The span before now.
        #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "5s")]
        last: Dur,
    },
    /// Record a user mark in the trace.
    Mark {
        /// The label.
        label: String,
    },
    /// Read or set the clipboard.
    #[command(subcommand)]
    Clipboard(ClipboardCommand),
    /// Read and drive the accessibility tree.
    #[command(subcommand)]
    A11y(super::a11y::A11yCommand),
    /// Record video of an output or a window.
    #[command(subcommand)]
    Record(super::media::RecordCommand),
    /// Print the latest CPU, memory and GPU samples. The session needs --sample.
    Resources,
    /// Step an output with manual timing.
    Step {
        /// The count of refreshes.
        #[arg(default_value_t = 1)]
        n: u32,
        /// The output id.
        #[arg(long, value_name = "ID")]
        output: Option<u32>,
    },
    /// Change an output.
    SetOutput {
        /// The output id. The default is the primary output.
        #[arg(long, value_name = "ID")]
        output: Option<u32>,
        /// A new mode as WIDTHxHEIGHT, with an optional @HZ refresh.
        #[arg(long, value_name = "MODE")]
        mode: Option<String>,
        /// A new scale.
        #[arg(long)]
        scale: Option<f64>,
        /// A new timing model.
        #[arg(long, value_name = "MODEL")]
        timing: Option<String>,
        /// Power the output on.
        #[arg(long)]
        enable: bool,
        /// Power the output off.
        #[arg(long, conflicts_with = "enable")]
        disable: bool,
    },
    /// Run a window operation: focus, maximize, fullscreen, close, suspend, resume,
    /// resize WxH, float X,Y,W,H or output ID.
    Window {
        /// The operation.
        op: String,
        /// The argument of resize, float and output.
        arg: Option<String>,
        /// The window.
        #[arg(long, value_name = "WINDOW", default_value = "focused")]
        window: String,
    },
    /// Print trace records as JSON lines until interrupted.
    Subscribe {
        /// Record types to print, such as presented or window_mapped. None prints every type.
        types: Vec<String>,
    },
    /// Call a method by name with JSON parameters.
    Raw {
        /// The method name.
        method: String,
        /// The parameters as JSON.
        params: Option<String>,
    },
    /// Run a bench preset or a scenario file as a bench in the session.
    Bench {
        #[command(flatten)]
        select: BenchSelect,
        /// Write the report as JSON.
        #[arg(long, value_name = "FILE")]
        report: Option<PathBuf>,
        /// Write a Chrome trace of the whole bench.
        #[arg(long, value_name = "FILE")]
        perfetto: Option<PathBuf>,
        /// Compare with a baseline report and exit with 3 on a regression.
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
    },
    /// List the bench presets of the server.
    Presets,
    /// Shut the session down and stop the server.
    Stop,
}

/// A `fakeland ctl wait` command.
#[derive(Debug, Subcommand)]
pub enum WaitCommand {
    /// Wait for a mapped window.
    Window {
        /// The window: focused, any, ID, app:APP_ID or title:REGEX.
        #[arg(default_value = "any")]
        window: String,
        /// The longest wait.
        #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "10s")]
        timeout: Dur,
    },
    /// Wait until a window shows no new content for the quiet time.
    Settled {
        /// The window.
        #[arg(default_value = "focused")]
        window: String,
        /// The quiet time.
        #[arg(long, value_name = "DUR", value_parser = parse_dur, default_value = "250ms")]
        quiet: Dur,
        /// The longest wait.
        #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "10s")]
        timeout: Dur,
    },
    /// Wait for presented frames of a window.
    Frames {
        /// The count of frames.
        n: u32,
        /// The window.
        #[arg(long, value_name = "WINDOW", default_value = "focused")]
        window: String,
        /// The longest wait.
        #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "10s")]
        timeout: Dur,
    },
}

/// A `fakeland ctl triggers` command.
#[derive(Debug, Subcommand)]
pub enum TriggersCommand {
    /// Arm the triggers of a JSON file: an array of triggers or `{"triggers": [...]}`.
    Install {
        /// The trigger file.
        file: PathBuf,
        /// The set ends after this time.
        #[arg(long, value_name = "DUR", value_parser = parse_secs, default_value = "1h")]
        lifetime: Dur,
    },
    /// Disarm a trigger set and print its firings.
    Remove {
        /// The set id.
        set: u64,
    },
}

/// A `fakeland ctl trace` command.
#[derive(Debug, Subcommand)]
pub enum TraceCommand {
    /// Start a trace capture.
    Start,
    /// End a trace capture and write it. The format follows the file name.
    Stop {
        /// The file: .jsonl, .json, .perfetto.json, .csv or .summary.json.
        file: PathBuf,
        /// The capture id. The default is the newest open capture.
        #[arg(long, value_name = "ID")]
        id: Option<u64>,
    },
    /// Write the buffered trace or its last span.
    Snapshot {
        /// The file.
        file: PathBuf,
        /// Keep only this last span.
        #[arg(long, value_name = "DUR", value_parser = parse_secs)]
        last: Option<Dur>,
    },
}

/// A `fakeland ctl clipboard` command.
#[derive(Debug, Subcommand)]
pub enum ClipboardCommand {
    /// Print the clipboard text.
    Get,
    /// Set the clipboard text.
    Set {
        /// The text.
        text: String,
    },
}
