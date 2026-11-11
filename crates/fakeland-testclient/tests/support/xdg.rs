//! xdg_wm_base, xdg_surface and xdg_toplevel for the test compositor. The configure goes out
//! after the first commit.

use wayland_protocols::xdg::shell::server::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New};

use super::state::State;

impl GlobalDispatch<XdgWmBase, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<XdgWmBase>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<XdgWmBase, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &XdgWmBase,
        request: xdg_wm_base::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let xdg_wm_base::Request::GetXdgSurface { id, .. } = request {
            state.xdg_surface = Some(data_init.init(id, ()));
        }
    }
}

impl Dispatch<XdgSurface, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &XdgSurface,
        request: xdg_surface::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let xdg_surface::Request::GetToplevel { id } = request {
            state.toplevel = Some(data_init.init(id, ()));
        }
    }
}

impl Dispatch<XdgToplevel, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &XdgToplevel,
        _: xdg_toplevel::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
