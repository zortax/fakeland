//! The events of the JSON log. Each line is one object with the `CLOCK_MONOTONIC` time `t` in
//! nanoseconds and the event name in `event`.

use serde::Serialize;

use super::InputDetail;
use crate::input::GroupKind;
use crate::scene::FrameState;

/// A global that the compositor advertises.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GlobalInfo {
    /// The interface name.
    pub interface: String,
    /// The advertised version.
    pub version: u32,
}

/// One log event.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LogEvent {
    /// The process started.
    Start {
        /// The process id.
        pid: u32,
        /// The crate version.
        version: String,
        /// The command line.
        args: Vec<String>,
    },
    /// The client connected and read the globals.
    Connected {
        /// Every advertised global.
        globals: Vec<GlobalInfo>,
        /// The compositor advertises fakeland_marks_v1 and the client uses it.
        marks: bool,
    },
    /// wp_presentation announced its clock.
    PresentationClock {
        /// The clock id; 1 is `CLOCK_MONOTONIC`.
        clock_id: u32,
    },
    /// A startup phase.
    Phase {
        /// The phase name.
        name: String,
    },
    /// The toplevel received a configure sequence.
    Configure {
        /// The xdg_surface serial.
        serial: u32,
        /// The suggested logical width; 0 leaves it to the client.
        width: i32,
        /// The suggested logical height; 0 leaves it to the client.
        height: i32,
        /// The toplevel states.
        states: Vec<String>,
    },
    /// The client acked a configure.
    Ack {
        /// The acked serial.
        serial: u32,
        /// The logical width the client uses from now on.
        width: i32,
        /// The logical height the client uses from now on.
        height: i32,
        /// Older pending serials that this ack replaces.
        superseded: Vec<u32>,
    },
    /// The buffer scale changed.
    Scale {
        /// `fractional`, `integer` or `none`.
        kind: String,
        /// The preferred scale in 120ths.
        scale120: u32,
        /// The integer wl_surface buffer scale.
        buffer_scale: i32,
    },
    /// The client dispatched one logical input event.
    InputReceived {
        /// The kind of the event.
        kind: GroupKind,
        /// The protocol time in milliseconds.
        wl_time: Option<u32>,
        /// The compositor's input id.
        marks_id: Option<u64>,
        /// When the event loop woke up for the read that delivered the event.
        wake: u64,
        /// The decoded effects.
        events: Vec<InputDetail>,
    },
    /// The client applied an input event to its state.
    InputApplied {
        /// The kind of the event.
        kind: GroupKind,
        /// The compositor's input id.
        marks_id: Option<u64>,
        /// The picture changed.
        changed: bool,
    },
    /// The compositor announced an input id that no handled event took.
    InputUnmatched {
        /// The compositor's input id.
        marks_id: u64,
        /// The announced kind.
        kind: Option<String>,
    },
    /// Rendering of a frame began.
    RenderBegin {
        /// The frame id.
        frame_id: u64,
    },
    /// Rendering of a frame ended. The commit follows at once.
    RenderEnd {
        /// The frame id.
        frame_id: u64,
        /// The frame got injected jank.
        janky: bool,
    },
    /// The client committed a frame and flushed the connection.
    Commit(Box<CommitInfo>),
    /// The client skipped frames.
    Skipped {
        /// `ticks` for passed fixed-rate ticks.
        reason: String,
        /// The number of skipped frames.
        count: u64,
    },
    /// A frame callback arrived.
    FrameDone {
        /// The frame whose commit requested the callback.
        frame_id: u64,
        /// The callback time in milliseconds.
        callback_ms: u32,
    },
    /// Presentation feedback reported a presented frame.
    Presented {
        /// The frame id.
        frame_id: u64,
        /// The presentation time in nanoseconds.
        time: u64,
        /// The refresh period in nanoseconds; 0 when unknown.
        refresh: u32,
        /// The vblank sequence.
        seq: u64,
        /// The wp_presentation_feedback kind flags.
        flags: u32,
    },
    /// Presentation feedback reported a discarded frame.
    Discarded {
        /// The frame id.
        frame_id: u64,
    },
    /// The compositor released a buffer.
    Release {
        /// The buffer number.
        buffer: u64,
        /// The frame that the buffer showed last.
        frame_id: u64,
    },
    /// The compositor asked the toplevel to close.
    Close,
    /// A non-fatal problem.
    Warning {
        /// The message.
        message: String,
    },
    /// The client exits.
    Exit {
        /// The process exit code.
        code: i32,
        /// Why the client exits.
        reason: String,
    },
}

/// The details of one commit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommitInfo {
    /// The frame id; it starts at 1.
    pub frame_id: u64,
    /// The buffer number.
    pub buffer: u64,
    /// The logical surface size `[w, h]`.
    pub size: [i32; 2],
    /// The buffer size in pixels `[w, h]`.
    pub buffer_size: [i32; 2],
    /// Buffer pixels per logical unit.
    pub scale: f64,
    /// The damage in buffer pixels `[x, y, w, h]`, or `None` for a frame equal to the last one.
    pub damage: Option<[i32; 4]>,
    /// The compositor input ids whose effect this frame shows for the first time.
    pub handled: Vec<u64>,
    /// The commit requests a frame callback.
    pub callback: bool,
    /// The commit requests presentation feedback.
    pub feedback: bool,
    /// The commit sets and waits for a fifo barrier.
    pub fifo: bool,
    /// The commit-timing target in nanoseconds.
    pub target: Option<u64>,
    /// When rendering began.
    pub render_begin: u64,
    /// When rendering ended.
    pub render_end: u64,
    /// What the frame shows.
    pub state: FrameState,
}
