//! Hit testing for input: the surface under a global point, with windows, subsurfaces and popups,
//! and the input id of an event with its notice to the app-mark managers of the target client.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_types::{InputEventId, SurfaceId, WindowId};
use smithay::desktop::WindowSurfaceType;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};

use crate::server::{Clients, State};

impl State {
    /// Returns the topmost surface under a global point with the global origin of that surface.
    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let (window, loc) = self.space.element_under(point)?;
        window
            .surface_under(point - loc.to_f64(), WindowSurfaceType::ALL)
            .map(|(surface, at)| (surface, (at + loc).to_f64()))
    }

    /// Returns the window under a global point.
    pub fn window_under(&self, point: Point<f64, Logical>) -> Option<WindowId> {
        let (window, _) = self.space.element_under(point)?;
        self.windows
            .iter()
            .find(|w| w.window == *window)
            .map(|w| w.id)
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
