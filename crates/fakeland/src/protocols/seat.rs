//! `wl_seat` "seat0": focus changes move the selection focus and land in the trace, and cursor
//! images land in the cursor state.

use fakeland_trace::Event;
use smithay::delegate_seat;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::selection::data_device::set_data_device_focus;
use smithay::wayland::selection::primary_selection::set_primary_focus;

use crate::server::State;

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.protocols.seat
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|s| self.dh.get_client(s.id()).ok());
        set_data_device_focus(&self.dh, seat, client.clone());
        set_primary_focus(&self.dh, seat, client);
        self.text_inputs.set_focus(focused);
        let surface = focused.and_then(|s| self.surfaces.id_of(s));
        self.emit(Event::Focus { surface });
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        if let CursorImageStatus::Surface(surface) = &image {
            self.surface_role(surface, "cursor", None);
        }
        self.windows.cursor.image = image;
    }
}

delegate_seat!(State);
