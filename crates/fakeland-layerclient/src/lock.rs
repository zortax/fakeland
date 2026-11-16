//! The session lock: the lock request, one lock surface per output, their configures, and unlock.

use serde_json::json;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::{
    self, ExtSessionLockSurfaceV1,
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_v1::{
    self, ExtSessionLockV1,
};

use crate::app::App;
use crate::log;

impl App {
    /// Locks the session and creates a lock surface on every output. Returns false without the
    /// lock manager.
    pub fn create_lock(&mut self, qh: &QueueHandle<Self>) -> bool {
        let Some(manager) = &self.lock_manager else {
            return false;
        };
        let lock = manager.lock(qh, ());
        for output in &self.outputs {
            let surface = self.compositor.create_surface(qh, ());
            let ls = lock.get_lock_surface(&surface, output, qh, ());
            self.lock_surfaces.push((surface, ls));
        }
        self.lock = Some(lock);
        true
    }

    /// Unlocks the session and ends the client.
    pub fn unlock(&mut self) {
        if let Some(lock) = self.lock.take() {
            lock.unlock_and_destroy();
            log::event("unlocked", json!({}));
        }
        self.done = true;
    }
}

impl Dispatch<ExtSessionLockV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => log::event("locked", json!({})),
            ext_session_lock_v1::Event::Finished => {
                log::event("finished", json!({}));
                app.done = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        ls: &ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            ls.ack_configure(serial);
            log::event(
                "lock_configure",
                json!({ "width": width, "height": height }),
            );
            let Some((surface, _)) = app.lock_surfaces.iter().find(|(_, l)| l == ls) else {
                return;
            };
            if let Err(e) = app.draw(surface, width.max(1), height.max(1), app.args.color, qh) {
                eprintln!("fakeland-layerclient: {e}");
            }
        }
    }
}
