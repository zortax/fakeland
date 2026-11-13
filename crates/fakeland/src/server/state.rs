//! The compositor state. One `State` lives on the compositor thread and owns every smithay object,
//! every faked output, the frame pipeline, windows, processes, timers and the event bus.

use fakeland_trace::Event;
use fakeland_types::{SharedClock, Timestamp};
use smithay::desktop::{PopupManager, Space, Window};
use smithay::input::Seat;
use smithay::reexports::calloop::{LoopHandle, LoopSignal};
use smithay::reexports::wayland_server::DisplayHandle;

use super::{Clients, Ids};
use crate::capture::{CaptureCost, CaptureJobs};
use crate::config::SessionConfig;
use crate::display::{DeadlineTimer, LatenessTracker, Timers};
use crate::framecap::FrameCaptures;
use crate::frames::{Frames, Surfaces};
use crate::input::{InputState, TextInputs};
use crate::launch::Processes;
use crate::marks::MarksServer;
use crate::outputs::Outputs;
use crate::protocols::Protocols;
use crate::reactor::Reactors;
use crate::recording::EventBus;
use crate::render::{CaptureTargets, RenderBackend, ShmReader};
use crate::runner::Runs;
use crate::sampling::{GpuGate, Sampling};
use crate::shell::Windows;
use crate::video::Recordings;
use crate::waits::{Observers, Waiters};

/// The compositor state.
pub struct State {
    /// The session clock.
    pub clock: SharedClock,
    /// The session configuration.
    pub config: SessionConfig,
    /// The display handle.
    pub dh: DisplayHandle,
    /// The event loop handle.
    pub loop_handle: LoopHandle<'static, State>,
    /// Stops the event loop.
    pub loop_signal: LoopSignal,
    /// The socket name in `$XDG_RUNTIME_DIR`.
    pub socket_name: String,
    /// Every protocol state.
    pub protocols: Protocols,
    /// The seat.
    pub seat: Seat<State>,
    /// The window space.
    pub space: Space<Window>,
    /// Popups.
    pub popups: PopupManager,
    /// The faked outputs.
    pub outputs: Outputs,
    /// Surface bookkeeping.
    pub surfaces: Surfaces,
    /// The frame pipeline.
    pub frames: Frames,
    /// Toplevel windows.
    pub windows: Windows,
    /// Connected clients.
    pub clients: Clients,
    /// Launched processes and the private bus.
    pub processes: Processes,
    /// Scheduled tasks.
    pub timers: Timers,
    /// The deadline timer.
    pub timer: DeadlineTimer,
    /// The event bus.
    pub bus: EventBus,
    /// In-thread record observers.
    pub observers: Observers,
    /// Pending waits.
    pub waiters: Waiters,
    /// The renderer.
    pub render: RenderBackend,
    /// The CPU renderer for shm buffer readbacks.
    pub shm_reader: ShmReader,
    /// The expected cost of captures.
    pub capture_cost: CaptureCost,
    /// The reused offscreen targets of captures.
    pub capture_targets: CaptureTargets,
    /// The stepped captures in flight.
    pub capture_jobs: CaptureJobs,
    /// Wake lateness.
    pub lateness: LatenessTracker,
    /// Id counters.
    pub ids: Ids,
    /// The input injector.
    pub input: InputState,
    /// The text inputs of every client.
    pub text_inputs: TextInputs,
    /// The app-mark managers.
    pub marks: MarksServer,
    /// Active frame captures.
    pub frame_captures: FrameCaptures,
    /// Active scenario runs.
    pub runs: Runs,
    /// Installed reactors.
    pub reactors: Reactors,
    /// Resource sampling, when the config turns it on.
    pub sampling: Option<Sampling>,
    /// The gate that the sampler thread holds during its GPU queries.
    pub gpu_gate: GpuGate,
    /// Running video recordings.
    pub recordings: Recordings,
    /// The nested viewer, while it runs.
    #[cfg(feature = "viewer")]
    pub viewer: Option<crate::viewer::ViewerLink>,
    /// The bytes of the selection the compositor serves, while it owns the clipboard.
    pub selection: Option<std::sync::Arc<[u8]>>,
    /// The space changed and needs a refresh.
    pub space_dirty: bool,
    /// The loop keeps running while this is true.
    pub running: bool,
}

impl State {
    /// Returns the current time.
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// Records an event at the current time.
    pub fn emit(&mut self, event: Event) -> Timestamp {
        let now = self.clock.now();
        self.bus.push(now, event)
    }

    /// Records an event at a given time.
    pub fn emit_at(&mut self, t: Timestamp, event: Event) -> Timestamp {
        self.bus.push(t, event)
    }

    /// Sends every queued event to the clients.
    pub fn flush_clients(&mut self) {
        if let Err(e) = self.dh.flush_clients() {
            tracing::debug!("client flush failed: {e}");
        }
    }
}
