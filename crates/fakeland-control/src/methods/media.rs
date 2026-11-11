//! Resource and recording methods: the latest resource samples, and video recordings of an
//! output or a window. Recording paths are absolute paths on the server host.

use fakeland::{RecordSpec, RecordingId, RecordingInfo, RecordingReport, ResourceSnapshot};
use serde::{Deserialize, Serialize};

use super::method::{NoParams, method};

/// A recording to stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordStopParams {
    /// The recording. `None` stops the newest recording.
    #[serde(default)]
    pub id: Option<RecordingId>,
}

method!(
    /// Returns the latest resource samples of every app and the GPU.
    Resources, "resources", NoParams => ResourceSnapshot
);
method!(
    /// Starts a recording and returns it.
    RecordStart, "record_start", RecordSpec => RecordingInfo
);
method!(
    /// Stops a recording, waits until the file is complete and returns its report.
    RecordStop, "record_stop", RecordStopParams => RecordingReport
);
method!(
    /// Returns every running recording.
    Recordings, "recordings", NoParams => Vec<RecordingInfo>
);
