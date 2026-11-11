//! wl_shm, wl_shm_pool and wl_buffer for the test compositor. The pixels are not read.

use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_shm::{self, WlShm};
use wayland_server::protocol::wl_shm_pool::{self, WlShmPool};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::state::State;

impl GlobalDispatch<WlShm, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlShm>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let shm = data_init.init(resource, ());
        shm.format(wl_shm::Format::Argb8888);
        shm.format(wl_shm::Format::Xrgb8888);
    }
}

impl Dispatch<WlShm, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlShm,
        request: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, .. } = request {
            data_init.init(id, ());
        }
    }
}

impl Dispatch<WlShmPool, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlShmPool,
        request: wl_shm_pool::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm_pool::Request::CreateBuffer { id, .. } = request {
            data_init.init(id, ());
        }
    }
}

impl Dispatch<WlBuffer, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlBuffer,
        _: <WlBuffer as Resource>::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
