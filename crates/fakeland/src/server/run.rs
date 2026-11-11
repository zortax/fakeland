//! The compositor thread: it builds the display, the event loop and the state, reports readiness
//! to the caller, then runs the loop until shutdown.

use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use fakeland_trace::{Environment, Event, TRACE_FORMAT_VERSION, TraceHeader};
use fakeland_types::SharedClock;
use smithay::desktop::{PopupManager, Space};
use smithay::input::keyboard::XkbConfig;
use smithay::reexports::calloop::channel::{self, Sender};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, Mode, PostAction};
use smithay::reexports::wayland_server::Display;
use smithay::wayland::socket::ListeningSocketSource;

use super::{Clients, Ids, State};
use crate::capture::{CaptureCost, CaptureJobs};
use crate::config::{DbusMode, SessionConfig};
use crate::display::{DeadlineTimer, LatenessTracker, Timers};
use crate::framecap::FrameCaptures;
use crate::frames::{Frames, Surfaces};
use crate::input::{InputState, KeymapLookup, TextInputs};
use crate::launch::{ChildEnv, DbusDaemon, Processes};
use crate::marks::MarksServer;
use crate::outputs::Outputs;
use crate::protocols::Protocols;
use crate::reactor::Reactors;
use crate::recording::{EventBus, detect};
use crate::render::{CaptureTargets, RenderBackend, ShmReader, default_cursor};
use crate::runner::Runs;
use crate::session::{Command, StartError};
use crate::shell::Windows;
use crate::video::Recordings;
use crate::waits::{Observers, Waiters};

/// What the caller learns once the socket listens.
pub struct Started {
    /// The command channel.
    pub tx: Sender<Command>,
    /// The compositor thread.
    pub thread: JoinHandle<()>,
    /// The socket name.
    pub socket_name: String,
    /// The environment of the trace header.
    pub environment: Environment,
    /// The private bus address.
    pub dbus_address: Option<String>,
}

struct Ready {
    tx: Sender<Command>,
    socket_name: String,
    environment: Environment,
    dbus_address: Option<String>,
}

/// Spawns the compositor thread and waits until it listens.
pub fn spawn(config: SessionConfig, clock: SharedClock) -> Result<Started, StartError> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<Ready, StartError>>();
    let thread = thread::Builder::new()
        .name("fakeland-compositor".into())
        .spawn(move || match init(config, clock) {
            Ok((event_loop, display, state, ready)) => {
                let _ = ready_tx.send(Ok(ready));
                run(event_loop, display, state);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        })
        .map_err(|e| StartError::Init(e.to_string()))?;
    let ready = ready_rx
        .recv()
        .map_err(|_| StartError::Init("the compositor thread ended".into()))??;
    Ok(Started {
        tx: ready.tx,
        thread,
        socket_name: ready.socket_name,
        environment: ready.environment,
        dbus_address: ready.dbus_address,
    })
}

type Parts = (EventLoop<'static, State>, Display<State>, State, Ready);

fn init(config: SessionConfig, clock: SharedClock) -> Result<Parts, StartError> {
    if let Err(e) =
        smithay::reexports::rustix::thread::set_current_timer_slack(std::num::NonZeroU64::new(1))
    {
        tracing::debug!("timer slack unchanged: {e}");
    }
    let dbus = match config.dbus {
        DbusMode::Private => Some(DbusDaemon::start()?),
        DbusMode::Host => None,
    };
    let dbus_address = dbus.as_ref().map(|d| d.address.clone());
    let a11y = crate::launch::start_a11y_bus(&config, dbus_address.as_deref());
    let event_loop: EventLoop<'static, State> =
        EventLoop::try_new().map_err(|e| StartError::Init(e.to_string()))?;
    let handle = event_loop.handle();
    let mut display: Display<State> =
        Display::new().map_err(|e| StartError::Init(e.to_string()))?;
    let dh = display.handle();

    let mut render = RenderBackend::open(config.renderer, config.render_node.as_deref())?;
    let shm_formats = render.shm_formats();
    let mut protocols = Protocols::new(&dh, &render, shm_formats, config.app_marks);
    let mut seat = protocols.seat.new_wl_seat(&dh, "seat0");
    let km = &config.input.keymap;
    seat.add_keyboard(
        XkbConfig {
            rules: &km.rules,
            model: &km.model,
            layout: &km.layout,
            variant: &km.variant,
            options: km.options.clone(),
        },
        config.input.repeat.delay_ms,
        config.input.repeat.rate,
    )
    .map_err(|e| StartError::Init(format!("keymap: {e}")))?;
    seat.add_pointer();
    let lookup = KeymapLookup::from_names(
        &km.rules,
        &km.model,
        &km.layout,
        &km.variant,
        km.options.clone(),
    )
    .map_err(|e| StartError::Init(format!("keymap: {e}")))?;
    if config.input.touch {
        seat.add_touch();
    }

    let socket = match &config.socket_name {
        Some(name) => ListeningSocketSource::with_name(name),
        None => ListeningSocketSource::new_auto(),
    }
    .map_err(|e| StartError::Socket(e.to_string()))?;
    let socket_name = socket.socket_name().to_string_lossy().into_owned();
    if let Some(daemon) = &dbus {
        let env = ChildEnv {
            socket: &socket_name,
            dbus_address: Some(&daemon.address),
            session_env: &config.env,
            launch_env: &[],
            accessibility: config.accessibility,
        }
        .build(std::env::vars());
        if let Err(e) = daemon.set_activation_env(&env) {
            tracing::warn!("activated services keep the minimal bus environment: {e}");
        }
    }
    handle
        .insert_source(socket, |stream, _, state| state.accept_client(stream))
        .map_err(|e| StartError::Socket(e.error.to_string()))?;

    let poll_fd: OwnedFd = display
        .backend()
        .poll_fd()
        .try_clone_to_owned()
        .map_err(|e| StartError::Init(e.to_string()))?;
    handle
        .insert_source(
            Generic::new(poll_fd, Interest::READ, Mode::Level),
            |_, _, _| Ok(PostAction::Continue),
        )
        .map_err(|e| StartError::Init(e.error.to_string()))?;

    let (timer, timer_fd) = DeadlineTimer::new().map_err(|e| StartError::Init(e.to_string()))?;
    handle
        .insert_source(
            Generic::new(timer_fd, Interest::READ, Mode::Level),
            |_, fd, state| {
                DeadlineTimer::drain(fd.as_ref());
                state.on_timer();
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| StartError::Init(e.error.to_string()))?;

    let (tx, commands) = channel::channel::<Command>();
    handle
        .insert_source(commands, |event, _, state| match event {
            channel::Event::Msg(cmd) => cmd(state),
            channel::Event::Closed => state.shutdown(),
        })
        .map_err(|e| StartError::Init(e.error.to_string()))?;
    let (exits_tx, exits) = channel::channel();
    handle
        .insert_source(exits, |event, _, state| {
            if let channel::Event::Msg(msg) = event {
                state.process_exited(msg);
            }
        })
        .map_err(|e| StartError::Init(e.error.to_string()))?;

    let environment = detect(&render.facts());
    let now = clock.now();
    let mut header = TraceHeader::new(environment.clone(), Vec::new(), now);
    header.viewer = config.viewer;
    let mut bus = EventBus::new(header, config.trace_capacity);
    bus.push(
        now,
        Event::SessionStarted {
            format_version: TRACE_FORMAT_VERSION,
            environment: environment.clone(),
        },
    );
    let outputs_config = config.outputs.clone();
    let mut state = State {
        clock,
        dh,
        loop_handle: handle,
        loop_signal: event_loop.get_signal(),
        socket_name: socket_name.clone(),
        protocols,
        seat,
        space: Space::default(),
        popups: PopupManager::default(),
        outputs: Outputs::default(),
        surfaces: Surfaces::default(),
        frames: Frames::default(),
        windows: Windows::new(default_cursor()),
        clients: Clients::default(),
        processes: Processes::new(exits_tx, dbus, config.log_capacity),
        timers: Timers::default(),
        timer,
        bus,
        observers: Observers::default(),
        waiters: Waiters::default(),
        render,
        shm_reader: ShmReader::default(),
        capture_cost: CaptureCost::default(),
        capture_targets: CaptureTargets::default(),
        capture_jobs: CaptureJobs::default(),
        lateness: LatenessTracker::new(config.lateness_threshold),
        ids: Ids::default(),
        input: InputState::new(lookup, config.input.pointer),
        marks: MarksServer::default(),
        text_inputs: TextInputs::default(),
        frame_captures: FrameCaptures::default(),
        runs: Runs::default(),
        reactors: Reactors::default(),
        sampling: None,
        recordings: Recordings::default(),
        #[cfg(feature = "viewer")]
        viewer: None,
        selection: None,
        space_dirty: false,
        running: true,
        config,
    };
    state.processes.a11y = a11y;
    for out in outputs_config {
        state
            .add_output(out)
            .map_err(|e| StartError::Init(e.to_string()))?;
    }
    if state.config.app_marks {
        state.install_marks_hook();
    }
    state.warm_up_capture();
    state
        .start_sampling()
        .map_err(|e| StartError::Init(format!("resource sampling: {e}")))?;
    #[cfg(feature = "viewer")]
    state.start_viewer().map_err(StartError::Viewer)?;
    #[cfg(not(feature = "viewer"))]
    if state.config.viewer {
        return Err(StartError::Viewer(
            "this build has no viewer; enable the viewer feature".into(),
        ));
    }
    state.rearm_timer();
    let ready = Ready {
        tx,
        socket_name,
        environment,
        dbus_address,
    };
    Ok((event_loop, display, state, ready))
}

fn run(mut event_loop: EventLoop<'static, State>, mut display: Display<State>, mut state: State) {
    while state.running {
        // Calloop waits for events before it runs idle callbacks, so a queued capture step
        // needs a poll that returns at once.
        let timeout = state.capture_jobs.has_queued().then_some(Duration::ZERO);
        if let Err(e) = event_loop.dispatch(timeout, &mut state) {
            tracing::error!("event loop failed: {e}");
            break;
        }
        if let Err(e) = display.dispatch_clients(&mut state) {
            tracing::warn!("client dispatch failed: {e}");
        }
        state.after_dispatch();
        if let Err(e) = display.flush_clients() {
            tracing::debug!("client flush failed: {e}");
        }
    }
    state.shutdown();
    let _ = display.flush_clients();
    drop(state);
    drop(display);
    drop(event_loop);
}

/// The grace span between SIGTERM and SIGKILL at shutdown.
pub const KILL_GRACE: Duration = Duration::from_millis(500);
