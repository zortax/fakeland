//! What a recording records and what it reports: the target, the file and its format, the
//! frame rate and the size limit.

use std::path::{Path, PathBuf};

use fakeland_scenario::WindowRef;
use fakeland_types::{Hz, Nanos, OutputId, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{FrameRate, RecordError};

/// The handle of a recording.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RecordingId(pub u64);

/// What a recording shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordTarget {
    /// Every window on an output.
    Output {
        /// The output. `None` selects the primary output.
        #[serde(default)]
        output: Option<OutputId>,
    },
    /// One window alone.
    Window {
        /// The window.
        #[serde(default)]
        window: WindowRef,
    },
}

impl Default for RecordTarget {
    fn default() -> Self {
        Self::Output { output: None }
    }
}

/// The file format of a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VideoFormat {
    /// H.264 through an `ffmpeg` child. The container follows the file extension: `.mp4`,
    /// `.mkv` or `.mov`.
    Mp4,
    /// One PNG file per frame in a directory, named `000000.png` and up. A repeated frame is a
    /// hard link to the previous file.
    PngSequence,
    /// An uncompressed YUV4MPEG2 stream in 4:2:0.
    Y4m,
}

impl VideoFormat {
    /// Picks the format of a path: `.y4m` is Y4m, `.mp4`, `.mkv` and `.mov` are Mp4, and a path
    /// without an extension is a PNG directory.
    pub fn from_path(path: &Path) -> Result<Self, RecordError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("y4m") => Ok(Self::Y4m),
            Some("mp4" | "mkv" | "mov") => Ok(Self::Mp4),
            None => Ok(Self::PngSequence),
            Some(other) => Err(RecordError::Format(format!(
                "unknown video extension .{other}; use .mp4, .mkv, .mov, .y4m or a directory"
            ))),
        }
    }

    /// Returns the snake_case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::PngSequence => "png_sequence",
            Self::Y4m => "y4m",
        }
    }
}

/// A recording request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordSpec {
    /// What the video shows.
    #[serde(default)]
    pub target: RecordTarget,
    /// The output file, or the directory of a PNG sequence.
    pub path: PathBuf,
    /// The format. `None` picks it from the path.
    #[serde(default)]
    pub format: Option<VideoFormat>,
    /// The frame rate. `None` takes one frame per refresh of the target output.
    #[serde(default)]
    pub fps: Option<Hz>,
    /// Downscale so the longer side is at most this many pixels.
    #[serde(default)]
    pub max_dim: Option<u32>,
    /// Draw the cursor.
    #[serde(default)]
    pub include_cursor: bool,
}

impl RecordSpec {
    /// Records the primary output into a file.
    pub fn output(path: impl Into<PathBuf>) -> Self {
        Self {
            target: RecordTarget::default(),
            path: path.into(),
            format: None,
            fps: None,
            max_dim: None,
            include_cursor: false,
        }
    }

    /// Records one window into a file.
    pub fn window(window: impl Into<WindowRef>, path: impl Into<PathBuf>) -> Self {
        Self {
            target: RecordTarget::Window {
                window: window.into(),
            },
            ..Self::output(path)
        }
    }

    /// Returns the spec on another output.
    pub fn on_output(mut self, output: OutputId) -> Self {
        self.target = RecordTarget::Output {
            output: Some(output),
        };
        self
    }

    /// Returns the spec with a format.
    pub fn format(mut self, format: VideoFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Returns the spec with a frame rate.
    pub fn fps(mut self, fps: Hz) -> Self {
        self.fps = Some(fps);
        self
    }

    /// Returns the spec with a size limit.
    pub fn max_dim(mut self, max_dim: u32) -> Self {
        self.max_dim = Some(max_dim);
        self
    }

    /// Returns the spec with the cursor drawn or hidden.
    pub fn cursor(mut self, on: bool) -> Self {
        self.include_cursor = on;
        self
    }

    /// Returns the format, from the spec or the path.
    pub fn resolved_format(&self) -> Result<VideoFormat, RecordError> {
        match self.format {
            Some(f) => Ok(f),
            None => VideoFormat::from_path(&self.path),
        }
    }
}

/// The result of a finished recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordingReport {
    /// The recording.
    pub id: RecordingId,
    /// The output file or directory.
    pub path: PathBuf,
    /// The format.
    pub format: VideoFormat,
    /// The frame width.
    pub width: u32,
    /// The frame height.
    pub height: u32,
    /// The frame rate.
    pub rate: FrameRate,
    /// The time of the first frame slot.
    pub first_slot: Timestamp,
    /// The time the recording stopped.
    pub stopped: Timestamp,
    /// Frames in the file.
    pub frames: u64,
    /// Frames that show a new composite.
    pub rendered: u64,
    /// Frames that repeat the previous picture, because the screen did not change.
    pub repeated: u64,
    /// Frames that repeat the previous picture because the writer fell behind.
    pub dropped: u64,
    /// The compositor time of every composite and readback.
    #[serde(with = "fakeland_types::dur::as_dur")]
    #[schemars(with = "fakeland_types::Dur")]
    pub capture_cost: Nanos,
    /// The video length: frames over the rate.
    #[serde(with = "fakeland_types::dur::as_dur")]
    #[schemars(with = "fakeland_types::Dur")]
    pub duration: Nanos,
    /// The first error of the writer or the encoder.
    pub error: Option<String>,
}

impl RecordingReport {
    /// Returns true when the file is complete.
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }

    /// Returns a one-line summary.
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "{} {}x{} @ {:.3} fps: {} frames ({:.2} s), {} rendered, {} repeated, {} dropped, capture {:.1} ms",
            self.path.display(),
            self.width,
            self.height,
            self.rate.hz().0,
            self.frames,
            self.duration.as_secs_f64(),
            self.rendered,
            self.repeated,
            self.dropped,
            self.capture_cost.as_millis_f64()
        );
        if let Some(e) = &self.error {
            s.push_str(&format!("; error: {e}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_follow_the_extension() {
        assert_eq!(
            VideoFormat::from_path(Path::new("a/b.MP4")).unwrap(),
            VideoFormat::Mp4
        );
        assert_eq!(
            VideoFormat::from_path(Path::new("x.y4m")).unwrap(),
            VideoFormat::Y4m
        );
        assert_eq!(
            VideoFormat::from_path(Path::new("frames")).unwrap(),
            VideoFormat::PngSequence
        );
        assert!(VideoFormat::from_path(Path::new("x.gif")).is_err());
    }

    #[test]
    fn spec_json_defaults_and_builders() {
        let s: RecordSpec = serde_json::from_str(r#"{"path":"/tmp/a.mp4"}"#).unwrap();
        assert_eq!(s, RecordSpec::output("/tmp/a.mp4"));
        assert_eq!(s.resolved_format().unwrap(), VideoFormat::Mp4);
        let s = RecordSpec::window(WindowRef::Focused, "/tmp/w.y4m")
            .fps(Hz(30.0))
            .max_dim(640)
            .cursor(true)
            .format(VideoFormat::PngSequence);
        let back: RecordSpec = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(s.resolved_format().unwrap(), VideoFormat::PngSequence);
    }
}
