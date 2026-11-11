//! The control methods. Each method is a marker type that implements [`Method`] with its wire
//! name, parameter type and result type. The client and the server share these types.

mod a11y;
mod apps;
mod bench;
mod capture;
mod clipboard;
pub mod defaults;
mod input;
mod media;
mod method;
mod outputs;
mod probes;
mod runs;
mod session;
mod subscribe;
mod trace;
mod triggers;
mod windows;

pub use a11y::{
    A11yActed, A11yAction, A11yActionParams, A11yApps, A11yFind, A11yFindParams, A11yFocus,
    A11yFound, A11yLimits, A11yNode, A11yNodeParams, A11ySetText, A11ySetTextParams, A11yTextSet,
    A11yTree, A11yTreeOf, A11yTreeParams,
};
pub use apps::{
    AppInfo, AppLogs, Apps, Kill, KillParams, KillSignal, Launch, LaunchParams, Logs, LogsParams,
    Restart, RestartParams,
};
pub use bench::{BenchOutcome, BenchParams, ListPresets, PresetEntry, RunBench};
pub use capture::{Screenshot, ScreenshotParams, ShotInfo};
pub use clipboard::{ClipboardGet, ClipboardGetParams, ClipboardSet, ClipboardSetParams};
pub use input::{
    Act, ActOutcome, ActParams, Click, ClickParams, ComboParams, DragParams, DragTo, Inject,
    InjectParams, KeyCombo, KeyParams, KeyTap, PointerMove, PointerMoveParams, Scroll,
    ScrollParams, TouchTap, TouchTapParams, TypeText, TypeTextParams,
};
pub use media::{RecordStart, RecordStop, RecordStopParams, Recordings, Resources};
pub use method::{Method, NoParams};
pub use outputs::{
    AddOutput, AddOutputParams, Outputs, RemoveOutput, RemoveOutputParams, SetOutput,
    SetOutputParams,
};
pub use probes::Probe;
pub use runs::{
    CancelRun, CancelRunParams, RunCapture, RunFrames, RunResultOf, RunResultParams, RunScenario,
    RunScenarioParams, RunStarted, RunState, RunSummary, Runs,
};
pub use session::{
    Info, Mark, MarkParams, ResetValidity, SessionInfo, Shutdown, Step, StepParams, Validity,
};
pub use subscribe::{Subscribe, SubscribeParams, SubscriptionInfo, Unsubscribe, UnsubscribeParams};
pub use trace::{
    FrameStats, FrameStatsOf, FrameStatsParams, TraceBegin, TraceCapture, TraceEnd, TraceEndParams,
    TraceFileInfo, TraceFormat, TraceOutput, TraceSnapshot, TraceSnapshotParams,
};
pub use triggers::{
    InstallTriggers, InstallTriggersParams, RemoveTriggers, RemoveTriggersParams, TriggerSet,
};
pub use windows::{
    Clients, WaitFor, WaitFrames, WaitFramesParams, WaitSettled, WaitSettledParams, WaitWindow,
    WaitWindowParams, WindowOpParams, WindowOperation, Windows, any_window,
};
