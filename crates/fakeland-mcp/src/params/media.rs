//! Parameters of the recording tools.

use schemars::JsonSchema;
use serde::Deserialize;

/// A recording to start.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordStartParams {
    /// The output file: `.mp4`, `.mkv` or `.mov` encode H.264, `.y4m` writes raw 4:2:0, and a
    /// path without extension becomes a directory of PNG frames. A relative path is relative to
    /// the working directory of the server.
    pub path: String,
    /// Record this window alone: `focused`, `any`, an id, `app:APP_ID` or `title:REGEX`. The
    /// default records the output.
    #[serde(default)]
    pub window: Option<String>,
    /// The output id. The default is the primary output.
    #[serde(default)]
    pub output: Option<u32>,
    /// The frame rate in Hz. The default is one frame per refresh of the output.
    #[serde(default)]
    pub fps: Option<f64>,
    /// Downscale so the longer side is at most this many pixels.
    #[serde(default)]
    pub max_dim: Option<u32>,
    /// Draw the cursor.
    #[serde(default)]
    pub cursor: bool,
}

/// A recording to stop.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct RecordStopParams {
    /// The recording id from record_start. The default is the newest recording.
    #[serde(default)]
    pub id: Option<u64>,
}
