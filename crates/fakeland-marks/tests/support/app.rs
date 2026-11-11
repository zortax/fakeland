//! The test app. It binds wl_compositor and a pointer on its own queue, owns a `Marks` handle
//! and takes the next input id inside its pointer motion handler.

use std::os::unix::net::UnixStream;

use fakeland_marks::{InputMark, Marks};
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};

/// The dispatch state of the test app.
pub struct AppState {
    pub marks: Marks,
    /// The result of `next_input` in each pointer motion handler, in order.
    pub motions: Vec<Option<InputMark>>,
    /// Whether the motion handler takes input ids.
    pub take_in_handler: bool,
}

/// A connected test app.
pub struct App {
    pub conn: Connection,
    pub queue: EventQueue<AppState>,
    pub state: AppState,
    pub compositor: WlCompositor,
    _pointer: WlPointer,
}

impl App {
    /// Connects over a socket and binds the core globals. `marks` builds the marks handle.
    pub fn connect(
        stream: UnixStream,
        marks: impl FnOnce(&Connection, &GlobalList) -> Marks,
    ) -> Self {
        let conn = Connection::from_socket(stream).expect("connection");
        let (globals, mut queue) = registry_queue_init::<AppState>(&conn).expect("globals");
        let qh = queue.handle();
        let compositor: WlCompositor = globals.bind(&qh, 4..=4, ()).expect("wl_compositor");
        let seat: WlSeat = globals.bind(&qh, 5..=5, ()).expect("wl_seat");
        let pointer = seat.get_pointer(&qh, ());
        let mut state = AppState {
            marks: marks(&conn, &globals),
            motions: Vec::new(),
            take_in_handler: true,
        };
        queue.roundtrip(&mut state).expect("roundtrip");
        Self {
            conn,
            queue,
            state,
            compositor,
            _pointer: pointer,
        }
    }

    /// Creates a surface on the app queue.
    pub fn surface(&self) -> WlSurface {
        self.compositor.create_surface(&self.queue.handle(), ())
    }

    /// Makes a roundtrip on the app queue.
    pub fn roundtrip(&mut self) {
        self.queue.roundtrip(&mut self.state).expect("roundtrip");
    }
}

impl Dispatch<WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Motion { .. } = event {
            let next = if state.take_in_handler {
                state.marks.next_input()
            } else {
                None
            };
            state.motions.push(next);
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for AppState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(AppState: ignore WlCompositor);
delegate_noop!(AppState: ignore WlSurface);
delegate_noop!(AppState: ignore WlSeat);
