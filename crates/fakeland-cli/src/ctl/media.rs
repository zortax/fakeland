//! `fakeland ctl record` and `fakeland ctl resources`: video recordings and resource samples of
//! the session.

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use fakeland::{RecordSpec, RecordTarget, RecordingId, RecordingInfo};
use fakeland_control::ControlClient;
use fakeland_control::methods::*;
use fakeland_control::spec::parse_window_ref;
use fakeland_types::{Hz, OutputId};

use super::args::CtlCommand;
use super::print::emit;
use crate::util::absolute;

/// A `fakeland ctl record` command.
#[derive(Debug, Subcommand)]
pub enum RecordCommand {
    /// Start a recording: `.mp4`, `.mkv` or `.mov` encode H.264 through ffmpeg, `.y4m` writes
    /// raw 4:2:0, and a path without extension becomes a directory of PNG frames.
    Start {
        /// The output file or directory.
        file: PathBuf,
        /// Record this window alone.
        #[arg(long, value_name = "WINDOW")]
        window: Option<String>,
        /// Record this output id. The default is the primary output.
        #[arg(long, value_name = "ID", conflicts_with = "window")]
        output: Option<u32>,
        /// The frame rate. The default is one frame per refresh.
        #[arg(long, value_name = "HZ")]
        fps: Option<f64>,
        /// Downscale so the longer side is at most this many pixels.
        #[arg(long, value_name = "PX")]
        max_dim: Option<u32>,
        /// Draw the cursor.
        #[arg(long)]
        cursor: bool,
    },
    /// Stop a recording and wait until its file is complete.
    Stop {
        /// The recording id. The default is the newest recording.
        id: Option<u64>,
    },
    /// List running recordings.
    List,
}

/// Returns the spec of `record start`.
pub fn record_spec(
    file: &std::path::Path,
    window: Option<&str>,
    output: Option<u32>,
    fps: Option<f64>,
    max_dim: Option<u32>,
    cursor: bool,
) -> Result<RecordSpec> {
    let target = match window {
        Some(w) => RecordTarget::Window {
            window: parse_window_ref(w)?,
        },
        None => RecordTarget::Output {
            output: output.map(OutputId),
        },
    };
    Ok(RecordSpec {
        target,
        path: absolute(file)?,
        format: None,
        fps: fps.map(Hz),
        max_dim,
        include_cursor: cursor,
    })
}

fn info_text(r: &RecordingInfo) -> String {
    format!(
        "recording {} -> {} ({}x{} @ {:.3} fps, {} frames, {} dropped)",
        r.id.0,
        r.path.display(),
        r.width,
        r.height,
        r.rate.hz().0,
        r.frames,
        r.dropped
    )
}

/// Runs the media commands.
pub fn run(client: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    match cmd {
        CtlCommand::Resources => {
            let snap = client.blocking_call::<Resources>(NoParams {})?;
            emit(json, &snap, |s| s.to_text())?;
        }
        CtlCommand::Record(RecordCommand::Start {
            file,
            window,
            output,
            fps,
            max_dim,
            cursor,
        }) => {
            let spec = record_spec(file, window.as_deref(), *output, *fps, *max_dim, *cursor)?;
            let info = client.blocking_call::<RecordStart>(spec)?;
            emit(json, &info, info_text)?;
        }
        CtlCommand::Record(RecordCommand::Stop { id }) => {
            let report = client.blocking_call::<RecordStop>(RecordStopParams {
                id: id.map(RecordingId),
            })?;
            emit(json, &report, |r| r.to_text())?;
            if !report.ok() {
                return Ok(Some(1));
            }
        }
        CtlCommand::Record(RecordCommand::List) => {
            let list = client.blocking_call::<Recordings>(NoParams {})?;
            emit(json, &list, |l| {
                if l.is_empty() {
                    "no recordings".into()
                } else {
                    l.iter().map(info_text).collect::<Vec<_>>().join("\n")
                }
            })?;
        }
        _ => return Ok(None),
    }
    Ok(Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_window_and_output_specs() {
        let s = record_spec(
            std::path::Path::new("/tmp/a.mp4"),
            Some("app:foot"),
            None,
            Some(30.0),
            Some(640),
            true,
        )
        .unwrap();
        assert!(matches!(s.target, RecordTarget::Window { .. }));
        assert_eq!(s.fps, Some(Hz(30.0)));
        assert!(s.include_cursor);
        let s = record_spec(
            std::path::Path::new("rel.y4m"),
            None,
            Some(2),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            s.target,
            RecordTarget::Output {
                output: Some(OutputId(2))
            }
        );
        assert!(s.path.is_absolute());
    }
}
