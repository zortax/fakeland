//! A virtual pointer on the test host compositor. It gives the host seat a pointer, so the
//! viewer window gets real `wl_pointer` events at layout positions.

use std::os::unix::net::UnixStream;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_pointer::ButtonState;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

struct Noop;

impl Dispatch<WlRegistry, GlobalListContents> for Noop {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(Noop: ignore WlSeat);
wayland_client::delegate_noop!(Noop: ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(Noop: ZwlrVirtualPointerV1);

/// A virtual pointer on the host seat.
pub struct HostPointer {
    conn: Connection,
    queue: EventQueue<Noop>,
    pointer: ZwlrVirtualPointerV1,
    extent: (u32, u32),
    time: u32,
}

impl HostPointer {
    /// Connects to the host socket and creates the pointer. `extent` is the layout size.
    pub fn new(socket: &str, extent: (u32, u32)) -> Self {
        let conn = Connection::from_socket(UnixStream::connect(socket).expect("host socket"))
            .expect("host connection");
        let (globals, mut queue) = registry_queue_init::<Noop>(&conn).expect("registry");
        let qh = queue.handle();
        let seat: WlSeat = globals.bind(&qh, 1..=7, ()).expect("seat");
        let manager: ZwlrVirtualPointerManagerV1 = globals
            .bind(&qh, 1..=2, ())
            .expect("virtual pointer manager");
        let pointer = manager.create_virtual_pointer(Some(&seat), &qh, ());
        queue.roundtrip(&mut Noop).expect("roundtrip");
        Self {
            conn,
            queue,
            pointer,
            extent,
            time: 1,
        }
    }

    fn tick(&mut self) -> u32 {
        self.time += 8;
        self.time
    }

    fn sync(&mut self) {
        self.pointer.frame();
        self.conn.flush().expect("flush");
        self.queue.roundtrip(&mut Noop).expect("roundtrip");
    }

    /// Moves to a layout position.
    pub fn move_to(&mut self, x: i64, y: i64) {
        let t = self.tick();
        self.pointer
            .motion_absolute(t, x as u32, y as u32, self.extent.0, self.extent.1);
        self.sync();
    }

    /// Clicks the left button.
    pub fn click(&mut self) {
        let t = self.tick();
        self.pointer.button(t, 0x110, ButtonState::Pressed);
        self.sync();
        let t = self.tick();
        self.pointer.button(t, 0x110, ButtonState::Released);
        self.sync();
    }
}
