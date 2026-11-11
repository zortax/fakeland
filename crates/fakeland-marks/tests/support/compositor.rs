//! wl_compositor and wl_surface for the test compositor. A commit applies and records the
//! pending marks of the surface.

use std::mem;

use wayland_server::protocol::wl_compositor::{self, WlCompositor};
use wayland_server::protocol::wl_surface::{self, WlSurface};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::record::Record;
use super::state::ServerState;

impl GlobalDispatch<WlCompositor, ()> for ServerState {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlCompositor>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WlCompositor, ()> for ServerState {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_compositor::Request::CreateSurface { id } = request {
            data_init.init(id, ());
        }
    }
}

impl Dispatch<WlSurface, ()> for ServerState {
    fn request(
        state: &mut Self,
        _: &Client,
        surface: &WlSurface,
        request: wl_surface::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wl_surface::Request::Commit = request {
            let marks = state
                .pending
                .get_mut(&surface.id())
                .map(mem::take)
                .unwrap_or_default();
            state.record(Record::Commit {
                surface: surface.id().protocol_id(),
                marks,
            });
        }
    }

    fn destroyed(
        state: &mut Self,
        _: wayland_server::backend::ClientId,
        surface: &WlSurface,
        _: &(),
    ) {
        state.pending.remove(&surface.id());
    }
}
