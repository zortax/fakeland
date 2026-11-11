//! wl_shm, wl_shm_pool and wl_buffer events: a buffer release frees its pool slot.

use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::BufferTag;
use crate::app::App;

wayland_client::delegate_noop!(App: ignore WlShm);
wayland_client::delegate_noop!(App: WlShmPool);

impl Dispatch<WlBuffer, BufferTag> for App {
    fn event(
        app: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        tag: &BufferTag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            app.buffer_released(*tag);
        }
    }
}
