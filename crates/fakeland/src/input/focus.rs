//! Hit testing for input: the surface under a global point across lock, layer and toplevel
//! surfaces with their subsurfaces and popups, and pointer refocus. The input id of an event goes
//! to the app-mark managers of the target client.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_types::{InputEventId, SurfaceId};
use smithay::input::pointer::MotionEvent;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::server::{Clients, State};

impl State {
    /// Returns the topmost surface that accepts input at a global point, with the global origin
    /// of that surface.
    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.hit(point).map(|h| (h.surface, h.origin))
    }

    /// Moves pointer focus to the surface under the pointer when it differs, so a button or axis
    /// event reaches the surface that the stacking shows there now.
    pub fn refocus_pointer(&mut self) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let position = self.windows.cursor.position;
        let under = self.surface_under(position);
        if pointer.current_focus() == under.as_ref().map(|(s, _)| s.clone()) {
            return;
        }
        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: position,
                serial: SERIAL_COUNTER.next_serial(),
                time: self.now().wayland_millis(),
            },
        );
        pointer.frame(self);
    }

    /// Allocates the id of an injected event and sends it to the app-mark managers of the client
    /// that owns `target`. Returns the id and the session id of the surface.
    pub fn mark_input(
        &mut self,
        target: Option<&WlSurface>,
        kind: MarkKind,
    ) -> (InputEventId, Option<SurfaceId>) {
        let id = self.ids.input();
        let surface = target.and_then(|s| self.surfaces.id_of(s));
        if let Some(client) = target
            .and_then(|s| s.client())
            .and_then(|c| Clients::id_of(&c))
        {
            self.marks.notify(client, id, kind);
        }
        (id, surface)
    }
}
