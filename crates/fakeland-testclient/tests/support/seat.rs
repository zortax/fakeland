//! wl_seat with a pointer for the test compositor.

use wayland_server::protocol::wl_pointer::{self, WlPointer};
use wayland_server::protocol::wl_seat::{self, WlSeat};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New};

use super::state::State;

impl GlobalDispatch<WlSeat, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlSeat>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let seat = data_init.init(resource, ());
        seat.capabilities(wl_seat::Capability::Pointer);
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlSeat,
        request: wl_seat::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_seat::Request::GetPointer { id } = request {
            state.pointers.push(data_init.init(id, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlPointer,
        _: wl_pointer::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
