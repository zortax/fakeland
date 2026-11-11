//! The trace event enum. Each variant is one fact that the compositor thread observes.

use fakeland_types::{
    ClientId, CommitId, InputEventId, Nanos, OutputId, Physical, ProbeId, ProcessId, Rect, RunId,
    SurfaceId, Timestamp, WindowId,
};
use serde::{Deserialize, Serialize};

use crate::event::{
    AppMarkKind, BufferInfo, Environment, InputKind, InputSource, OutputInfo, SyncKind,
};

/// One trace event. The JSON form is internally tagged with a snake_case `type` field.
///
/// Variants are append-only. A reader of an older build reads an unknown variant as `Unknown`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Event {
    /// The session starts.
    SessionStarted {
        /// The trace format version of the writer.
        format_version: u32,
        /// The host environment.
        environment: Environment,
    },
    /// An output appears with its configuration.
    OutputConfigured(OutputInfo),
    /// An output changes mode, scale, transform or timing.
    OutputChanged(OutputInfo),
    /// An output disappears.
    OutputRemoved {
        /// The output.
        output: OutputId,
    },
    /// An output turns on or off. An output that is off has no vblanks.
    OutputPower {
        /// The output.
        output: OutputId,
        /// True when the output is on.
        on: bool,
    },
    /// A vblank of the display clock model. The record time is the wake time.
    Vblank {
        /// The output.
        output: OutputId,
        /// The vblank sequence number.
        seq: u64,
        /// The ideal vblank time on the model grid.
        ideal: Timestamp,
        /// The time the compositor thread woke for the vblank.
        woke: Timestamp,
        /// True when the perturbation model drops this vblank.
        dropped: bool,
    },
    /// A Wayland client connects.
    ClientConnected {
        /// The client.
        client: ClientId,
        /// The client process from `SO_PEERCRED`.
        pid: Option<ProcessId>,
    },
    /// A Wayland client disconnects.
    ClientDisconnected {
        /// The client.
        client: ClientId,
    },
    /// The launcher starts a process.
    ProcessSpawned {
        /// The process.
        pid: ProcessId,
        /// The command line.
        command: String,
    },
    /// A process exits.
    ProcessExited {
        /// The process.
        pid: ProcessId,
        /// The exit code of a normal exit.
        code: Option<i32>,
        /// The signal that ended the process.
        #[serde(default)]
        signal: Option<i32>,
    },
    /// A client creates a surface.
    SurfaceCreated {
        /// The surface.
        surface: SurfaceId,
        /// The owner.
        client: ClientId,
    },
    /// A surface is destroyed.
    SurfaceDestroyed {
        /// The surface.
        surface: SurfaceId,
    },
    /// A surface gets a role.
    SurfaceRole {
        /// The surface.
        surface: SurfaceId,
        /// The role, for example `xdg_toplevel`, `xdg_popup`, `subsurface` or `cursor`.
        role: String,
        /// The parent surface of a subsurface or popup.
        #[serde(default)]
        parent: Option<SurfaceId>,
    },
    /// A toplevel window maps.
    WindowMapped {
        /// The window.
        window: WindowId,
        /// The toplevel surface.
        surface: SurfaceId,
        /// The xdg app id.
        app_id: String,
        /// The title.
        title: String,
        /// The output that shows the window.
        output: Option<OutputId>,
    },
    /// A toplevel window unmaps.
    WindowUnmapped {
        /// The window.
        window: WindowId,
    },
    /// A window changes its title.
    WindowTitle {
        /// The window.
        window: WindowId,
        /// The new title.
        title: String,
    },
    /// A window changes its app id.
    WindowAppId {
        /// The window.
        window: WindowId,
        /// The new app id.
        app_id: String,
    },
    /// Keyboard focus moves.
    Focus {
        /// The surface with keyboard focus, or `None` when no surface has focus.
        surface: Option<SurfaceId>,
    },
    /// The compositor sends a configure.
    Configure {
        /// The xdg surface.
        surface: SurfaceId,
        /// The configure serial.
        serial: u32,
        /// The suggested width in logical units. Zero lets the client choose.
        width: i32,
        /// The suggested height in logical units. Zero lets the client choose.
        height: i32,
        /// The toplevel states, for example `maximized`, `activated` or `suspended`.
        states: Vec<String>,
    },
    /// A client acknowledges a configure.
    AckConfigure {
        /// The xdg surface.
        surface: SurfaceId,
        /// The acknowledged serial.
        serial: u32,
    },
    /// A surface commit applies.
    Commit {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
        /// The newly attached buffer. `None` means the commit attaches no new buffer.
        buffer: Option<BufferInfo>,
        /// The buffer damage in buffer coordinates.
        damage: Vec<Rect<Physical>>,
        /// True when the commit requests a frame callback.
        frame_callback: bool,
        /// True when the commit requests presentation feedback.
        presentation_feedback: bool,
        /// The GPU synchronization of the buffer.
        sync: SyncKind,
        /// True when the commit sets a fifo barrier.
        fifo_barrier: bool,
        /// True when the commit waits for the previous fifo barrier.
        fifo_wait: bool,
        /// The commit-timing target time.
        target: Option<Timestamp>,
        /// The serial of the last configure that the client acknowledged before this commit.
        configure_serial: Option<u32>,
        /// True when the commit attaches a null buffer and unmaps the surface.
        #[serde(default)]
        detached: bool,
    },
    /// The fences of a commit signal. The buffer is ready to show.
    Ready {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
    },
    /// A commit latches for a vblank.
    Latched {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
        /// The output.
        output: OutputId,
        /// The vblank sequence number the commit shows on.
        seq: u64,
    },
    /// A commit is presented. The compositor sends `wp_presentation_feedback.presented`.
    Presented {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
        /// The output.
        output: OutputId,
        /// The vblank sequence number.
        seq: u64,
        /// The presentation time.
        time: Timestamp,
        /// The refresh period, or `None` for an output without fixed refresh.
        refresh: Option<Nanos>,
        /// The `wp_presentation_feedback` kind flags.
        flags: u32,
    },
    /// A newer commit replaces a commit before it shows.
    Discarded {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
    },
    /// The compositor releases the buffer of a commit.
    Released {
        /// The surface.
        surface: SurfaceId,
        /// The commit.
        commit: CommitId,
    },
    /// The compositor sends a frame callback.
    FrameCallbackSent {
        /// The surface.
        surface: SurfaceId,
        /// The time in the callback.
        time: Timestamp,
    },
    /// The compositor sends an input event and flushes the client socket.
    InputSent {
        /// The input event.
        id: InputEventId,
        /// The kind.
        kind: InputKind,
        /// The surface that receives the event.
        surface: Option<SurfaceId>,
        /// The global logical horizontal position.
        x: Option<f64>,
        /// The global logical vertical position.
        y: Option<f64>,
        /// A short description, for example a button, key or axis value.
        detail: String,
        /// The origin.
        source: InputSource,
    },
    /// A client attaches an app mark.
    AppMark {
        /// The surface.
        surface: Option<SurfaceId>,
        /// The commit that carries the mark.
        commit: Option<CommitId>,
        /// The mark.
        mark: AppMarkKind,
    },
    /// A user sets a mark through the API.
    UserMark {
        /// The label.
        label: String,
    },
    /// A scenario run starts.
    ScenarioStarted {
        /// The run.
        run: RunId,
        /// The scenario name.
        name: String,
    },
    /// A scenario run reaches a step.
    ScenarioStep {
        /// The run.
        run: RunId,
        /// The step index.
        index: u32,
        /// The step label.
        label: String,
    },
    /// A scenario run ends.
    ScenarioFinished {
        /// The run.
        run: RunId,
        /// True when every step succeeds.
        ok: bool,
        /// The error of a failed run.
        error: Option<String>,
    },
    /// A latency probe arms.
    ProbeStarted {
        /// The probe.
        probe: ProbeId,
        /// The probe kind, for example `pointer_move` or `key`.
        kind: String,
    },
    /// A latency probe sends its input event.
    ProbeInput {
        /// The probe.
        probe: ProbeId,
        /// The input event.
        input: InputEventId,
    },
    /// A latency probe detects its response in a commit.
    ProbeDetected {
        /// The probe.
        probe: ProbeId,
        /// The commit that shows the response.
        commit: CommitId,
        /// The detector, for example `any_damage` or `pixels_changed`.
        detector: String,
    },
    /// A latency probe sees no response before its timeout.
    ProbeTimedOut {
        /// The probe.
        probe: ProbeId,
    },
    /// The compositor captures pixels.
    Capture {
        /// The target, for example `output DP-1` or `roi 10,10 64x64`.
        what: String,
        /// The time the capture costs the compositor thread.
        cost: Nanos,
    },
    /// The compositor thread spends time on work outside the frame pipeline.
    CompositorOverhead {
        /// The work.
        what: String,
        /// The time the work takes. The work ends at the record time.
        duration: Nanos,
    },
    /// A resource sample of a process tree.
    ProcessSample {
        /// The root process.
        pid: ProcessId,
        /// CPU use in percent of one core.
        cpu_percent: f64,
        /// Resident memory in bytes.
        rss_bytes: u64,
        /// The thread count.
        threads: u32,
    },
    /// A GPU resource sample.
    GpuSample {
        /// GPU utilization in percent.
        util_percent: f64,
        /// Used GPU memory in bytes.
        mem_used_bytes: u64,
        /// Total GPU memory in bytes.
        mem_total_bytes: u64,
    },
    /// The clipboard or primary selection changes.
    Clipboard {
        /// The offered mime types.
        mime: Vec<String>,
        /// True for the primary selection.
        #[serde(default)]
        primary: bool,
    },
    /// The compositor thread wakes for an output deadline whose earliest step is no vblank: a
    /// latch, an apply of updates without content, or a frame callback dispatch. The record time
    /// is the wake time.
    DeadlineWake {
        /// The output.
        output: OutputId,
        /// The earliest deadline the wake processes.
        deadline: Timestamp,
        /// The time the compositor thread woke for the deadline.
        woke: Timestamp,
    },
    /// An event of a newer format that this build does not know.
    #[serde(other)]
    Unknown,
}
