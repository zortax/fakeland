//! H.264 through an `ffmpeg` child: raw RGBA frames go to its stdin, and its stderr goes to a
//! reader thread, so a full pipe never stalls it.

use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::JoinHandle;

use fakeland_vision::ImageView;

use super::Sink;
use crate::video::{Canvas, FrameRate};

/// Returns the ffmpeg arguments of a recording.
pub fn args(path: &Path, canvas: Canvas, rate: FrameRate) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-video_size",
        &format!("{}x{}", canvas.width, canvas.height),
        "-framerate",
        &rate.ffmpeg(),
        "-i",
        "pipe:0",
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "18",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        "-y",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(std::iter::once(path.to_string_lossy().into_owned()))
    .collect()
}

/// An ffmpeg encoder.
pub struct FfmpegSink {
    child: Child,
    stdin: Option<BufWriter<ChildStdin>>,
    stderr: Option<JoinHandle<String>>,
    width: usize,
}

impl FfmpegSink {
    /// Starts ffmpeg.
    pub fn open(path: &Path, canvas: Canvas, rate: FrameRate) -> Result<Self, String> {
        let mut child = Command::new("ffmpeg")
            .args(args(path, canvas, rate))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start ffmpeg: {e}"))?;
        let stdin = child
            .stdin
            .take()
            .map(|s| BufWriter::with_capacity(1 << 20, s));
        let stderr = child.stderr.take().and_then(|mut err| {
            std::thread::Builder::new()
                .name("fakeland-ffmpeg-log".into())
                .spawn(move || {
                    let mut s = String::new();
                    let _ = err.read_to_string(&mut s);
                    s
                })
                .ok()
        });
        Ok(Self {
            child,
            stdin,
            stderr,
            width: canvas.width as usize,
        })
    }

    fn log(&mut self) -> String {
        self.stderr
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}

impl Sink for FfmpegSink {
    fn write(&mut self, frame: &ImageView<'_>, count: u64) -> Result<(), String> {
        let Some(stdin) = &mut self.stdin else {
            return Err("ffmpeg input is closed".into());
        };
        let bytes = self.width * 4;
        for _ in 0..count {
            for row in frame.rows() {
                if let Err(e) = stdin.write_all(&row[..bytes]) {
                    self.stdin = None;
                    let _ = self.child.wait();
                    let log = self.log();
                    return Err(format!("ffmpeg stopped: {e}: {log}"));
                }
            }
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<(), String> {
        if let Some(mut stdin) = self.stdin.take() {
            stdin.flush().map_err(|e| format!("ffmpeg input: {e}"))?;
        }
        let status = self.child.wait().map_err(|e| format!("ffmpeg: {e}"))?;
        let log = self.log();
        if status.success() {
            Ok(())
        } else {
            Err(format!("ffmpeg failed with {status}: {log}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_describe_the_raw_input() {
        let a = args(
            Path::new("/tmp/out.mp4"),
            Canvas::of(640, 360),
            FrameRate::new(60, 1).unwrap(),
        );
        let joined = a.join(" ");
        assert!(joined.contains("-f rawvideo -pix_fmt rgba -video_size 640x360 -framerate 60/1"));
        assert!(joined.contains("-c:v libx264"));
        assert_eq!(a.last().unwrap(), "/tmp/out.mp4");
    }
}
