//! The client state and its construction: connect, bind the globals, create the toplevel and
//! set up marks, scenes and pacing.

use fakeland_marks::{Marks, SurfaceMarks};
use fakeland_types::{Clock, InputEventId, Logical, MonotonicClock, Physical, Size, Timestamp};
use wayland_client::globals::registry_queue_init;
use wayland_client::{Connection, EventQueue, QueueHandle};

use super::{AnimInputs, ConfigureQueue, ExitReason, PendingConfigure};
use crate::cli::Args;
use crate::input::{DelayQueue, InputGroup, MarkMatcher};
use crate::load::LoadPlan;
use crate::log::{EventLog, LogEvent};
use crate::pace::Pacer;
use crate::scene::SceneSet;
use crate::wl::{
    BufferPool, Globals, Outputs, Scale, ScaleState, SeatState, SurfaceObjects, SurfaceSetup,
};

/// The toplevel configure that waits for its xdg_surface.configure.
#[derive(Clone, Debug, Default)]
pub struct ToplevelConfigure {
    /// The suggested width.
    pub width: i32,
    /// The suggested height.
    pub height: i32,
    /// The state names.
    pub states: Vec<String>,
}

/// The window size and the configures that wait for their ack.
#[derive(Debug, Default)]
pub struct WindowState {
    /// The acked logical size.
    pub size: Option<Size<Logical>>,
    /// The toplevel part of the next configure.
    pub toplevel: Option<ToplevelConfigure>,
    /// The configures that wait for the configure delay.
    pub queue: ConfigureQueue,
    /// The scale in the last scale log event.
    pub logged_scale: Option<Scale>,
}

/// The app marks of the session.
#[derive(Debug)]
pub struct Marking {
    /// The marks manager.
    pub marks: Marks,
    /// The marks object of the surface.
    pub surface: Option<SurfaceMarks>,
    /// The input ids that wait for their event.
    pub matcher: MarkMatcher,
    /// The ids whose effect the next frame shows.
    pub handled: Vec<InputEventId>,
}

/// The render state.
#[derive(Debug)]
pub struct RenderState {
    /// The buffers.
    pub pool: BufferPool,
    /// The frame being drawn.
    pub scratch: Vec<u8>,
    /// The last committed frame.
    pub last: Vec<u8>,
    /// The buffer size of the last committed frame.
    pub last_size: Option<Size<Physical>>,
    /// The id of the last committed frame.
    pub frame_id: u64,
    /// The scale and logical size set on the surface.
    pub applied: Option<(Scale, Size<Logical>)>,
    /// Presentation feedback requests without an answer.
    pub feedback_pending: usize,
    /// The time inputs of the animation.
    pub anim: AnimInputs,
}

/// The whole client.
#[derive(Debug)]
pub struct App {
    /// The command line.
    pub args: Args,
    /// The session clock.
    pub clock: MonotonicClock,
    /// The event log.
    pub log: EventLog,
    /// The connection.
    pub conn: Connection,
    /// The handle of the main queue.
    pub qh: QueueHandle<App>,
    /// The bound globals.
    pub globals: Globals,
    /// The surface objects.
    pub surface: SurfaceObjects,
    /// The outputs.
    pub outputs: Outputs,
    /// The scale hints.
    pub scale: ScaleState,
    /// The seat objects.
    pub seat: SeatState,
    /// The render state.
    pub render: RenderState,
    /// The scenes.
    pub scenes: SceneSet,
    /// The frame pacing.
    pub pacer: Pacer,
    /// The injected load.
    pub load: LoadPlan,
    /// The window state.
    pub window: WindowState,
    /// The app marks.
    pub marking: Marking,
    /// The inputs that wait for the input delay.
    pub inputs: DelayQueue<InputGroup>,
    /// When the event loop last woke up.
    pub wake: Timestamp,
    /// The exit that the loop takes next.
    pub exit: Option<ExitReason>,
    /// The `--exit-after` deadline.
    pub exit_at: Option<Timestamp>,
    /// The deadline for the feedback of the last frame after `--exit-after-frames`.
    pub drain_until: Option<Timestamp>,
    /// The loop must not block, because a frame wants to render.
    pub again: bool,
}

impl App {
    /// Connects to the compositor and creates the toplevel. The app takes the log on success;
    /// on failure the log stays with the caller.
    pub fn connect(
        args: Args,
        log: &mut EventLog,
        started: Timestamp,
    ) -> Result<(Self, EventQueue<Self>), ExitReason> {
        let clock = MonotonicClock;
        let conn =
            Connection::connect_to_env().map_err(|e| ExitReason::Disconnected(e.to_string()))?;
        let (globals, queue) = registry_queue_init::<Self>(&conn)
            .map_err(|e| ExitReason::Disconnected(e.to_string()))?;
        let qh = queue.handle();
        let bound =
            Globals::bind(&globals, &qh).map_err(|e| ExitReason::MissingGlobal(e.0.to_owned()))?;
        let marks = if args.no_marks {
            Marks::inactive()
        } else {
            Marks::bind(&conn, &globals)
        };
        log.write(
            clock.now(),
            &LogEvent::Connected {
                globals: bound.list.clone(),
                marks: marks.is_active(),
            },
        );
        let strategy = ScaleState::pick(
            args.scale_mode,
            bound.fractional.is_some() && bound.viewporter.is_some(),
        )
        .ok_or_else(|| ExitReason::MissingGlobal("wp_fractional_scale_manager_v1".into()))?;
        let setup = SurfaceSetup {
            app_id: &args.app_id,
            title: &args.title,
            strategy,
            fifo: args.fifo,
            commit_timing: args.target_delay.is_some(),
        };
        let surface = SurfaceObjects::create(&bound, &qh, &setup)
            .map_err(|e| ExitReason::MissingGlobal(e.0.to_owned()))?;
        let surface_marks = marks.surface(&surface.surface).ok();
        let mut outputs = Outputs::default();
        outputs.bind_all(&globals, &qh);
        let scene_options = args.scene_options();
        let scenes = SceneSet::new(&scene_options);
        let pacer = Pacer::new(args.mode, args.rate_hz(), scenes.idle(), scenes.animating());
        let pool = BufferPool::new(bound.shm.clone(), args.buffers as usize);
        let exit_at = args.exit_after.map(|d| started + d);
        let load = args.load_plan();
        let mut app = Self {
            clock,
            log: std::mem::replace(log, EventLog::disabled()),
            conn,
            qh,
            surface,
            outputs,
            scale: ScaleState::new(strategy),
            seat: SeatState::default(),
            render: RenderState {
                pool,
                scratch: Vec::new(),
                last: Vec::new(),
                last_size: None,
                frame_id: 0,
                applied: None,
                feedback_pending: 0,
                anim: AnimInputs::default(),
            },
            scenes,
            pacer,
            load,
            window: WindowState::default(),
            marking: Marking {
                marks,
                surface: surface_marks,
                matcher: MarkMatcher::default(),
                handled: Vec::new(),
            },
            inputs: DelayQueue::default(),
            wake: started,
            exit: None,
            exit_at,
            drain_until: None,
            again: false,
            globals: bound,
            args,
        };
        app.phase("connected");
        if app.globals.seat.is_none() {
            app.warn("the compositor offers no wl_seat; the client gets no input".to_owned());
        }
        Ok((app, queue))
    }

    /// Records a startup phase in the log and as a client-level app mark.
    pub fn mark(&self, name: &str) {
        // A phase name is a short constant, so the marks crate accepts it.
        let _ = self.marking.marks.mark(name);
    }

    /// Records a startup phase.
    pub fn phase(&mut self, name: &str) {
        self.mark(name);
        let now = self.clock.now();
        self.log.write(
            now,
            &LogEvent::Phase {
                name: name.to_owned(),
            },
        );
    }

    /// Logs a non-fatal problem.
    pub fn warn(&mut self, message: String) {
        let now = self.clock.now();
        self.log.write(now, &LogEvent::Warning { message });
    }

    /// Asks the loop to exit. The first reason wins.
    pub fn request_exit(&mut self, reason: ExitReason) {
        self.exit.get_or_insert(reason);
    }

    /// Returns the acked logical size, or the default size before the first configure.
    pub fn logical_size(&self) -> Size<Logical> {
        self.window.size.unwrap_or(self.args.size)
    }

    /// Returns the configure that a toplevel configure and its serial form.
    pub fn pending_configure(&mut self, serial: u32) -> PendingConfigure {
        let top = self.window.toplevel.take().unwrap_or_default();
        PendingConfigure {
            serial,
            width: top.width,
            height: top.height,
            states: top.states,
        }
    }
}
