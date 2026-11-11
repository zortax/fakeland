//! Tool parameters. Every field has a description for the agent and a default where one makes
//! sense.

mod a11y;
mod defaults;
mod input;
mod measure;
mod media;
mod screen;
mod session;

pub use a11y::{
    A11yActionParams, A11yFindParams, A11yScopeParams, A11ySetTextParams, A11yTreeParams,
};
pub use input::{ClickParams, DragParams, KeyParams, PointParams, ScrollParams, TypeTextParams};
pub use measure::{
    BenchmarkParams, FrameStatsParams, LatencyAction, LatencyDetector, MeasureLatencyParams,
    RunScenarioParams, TraceStartParams, TraceStopParams,
};
pub use media::{RecordStartParams, RecordStopParams};
pub use screen::{Region, ScreenshotParams, WaitForParams, WaitKind};
pub use session::{
    ClipboardSetParams, KillAppParams, LaunchParams, LogsParams, NoArgs, RestartAppParams,
    SessionStartParams, SetOutputParams, WindowParams,
};
