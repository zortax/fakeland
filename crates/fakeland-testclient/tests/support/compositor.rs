//! wl_compositor, wl_surface and wl_callback for the test compositor. A commit records the
//! surface marks, releases the replaced buffer and answers frame callbacks at once.

use std::mem;

use wayland_server::protocol::wl_callback::WlCallback;
use wayland_server::protocol::wl_compositor::{self, WlCompositor};
use wayland_server::protocol::wl_surface::{self, WlSurface};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::state::{Commit, State};

impl GlobalDispatch<WlCompositor, ()> for State {
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

impl Dispatch<WlCompositor, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_compositor::Request::CreateSurface { id } = request {
            let surface = data_init.init(id, ());
            state.surface = Some(surface);
        }
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        surface: &WlSurface,
        request: wl_surface::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let pending = state.pending.entry(surface.id()).or_default();
        match request {
            wl_surface::Request::Attach { buffer, .. } => {
                pending.buffer = buffer;
                pending.attached = true;
            }
            wl_surface::Request::Frame { callback } => {
                pending.callbacks.push(data_init.init(callback, ()));
            }
            wl_surface::Request::Commit => commit(state, surface),
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlCallback,
        _: <WlCallback as Resource>::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

fn commit(state: &mut State, surface: &WlSurface) {
    let pending = state.pending.entry(surface.id()).or_default();
    let attached = mem::take(&mut pending.attached);
    let buffer = pending.buffer.take();
    let callbacks = mem::take(&mut pending.callbacks);
    let timing = mem::take(&mut pending.timing);
    let marks = state
        .marks
        .get_mut(&surface.id())
        .map(mem::take)
        .unwrap_or_default();
    if attached {
        if let Some(old) = state.shown.take() {
            old.release();
        }
        state.shown = buffer;
    }
    for cb in callbacks {
        cb.done(0);
    }
    state.commits.push(Commit {
        buffer: attached && state.shown.is_some(),
        marks,
        timing,
    });
    if !state.configured
        && let (Some(top), Some(xdg)) = (&state.toplevel, &state.xdg_surface)
    {
        state.configured = true;
        top.configure(400, 300, Vec::new());
        let serial = state.serial + 1;
        state.serial = serial;
        xdg.configure(serial);
    }
}
