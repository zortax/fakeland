//! The session lock: the lock request, one lock surface per output and per new output, their
//! configures, unlock on input, and relock.

use serde_json::json;
use wayland_client::protocol::wl_output::WlOutput;
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
        self.lock = Some(lock);
        for output in self.outputs.clone() {
            self.add_lock_surface(&output, qh);
        }
        true
    }

    /// Creates a lock surface on an output while the client holds a lock object.
    pub fn add_lock_surface(&mut self, output: &WlOutput, qh: &QueueHandle<Self>) {
        let Some(lock) = &self.lock else {
            return;
        };
        let surface = self.compositor.create_surface(qh, ());
        let ls = lock.get_lock_surface(&surface, output, qh, ());
        self.lock_surfaces.push((surface, ls));
    }

    /// Handles a button or key press: the first press relocks with `--relock-on-input`, and a
    /// press unlocks with `--unlock-on-input`.
    pub fn lock_input(&mut self, qh: &QueueHandle<Self>) {
        if self.args.relock_on_input && !self.relocked && self.lock.is_some() {
            self.relock(qh);
        } else if self.args.unlock_on_input {
            self.unlock();
        }
    }

    /// Unlocks the session and locks it again before the old lock surfaces go away.
    fn relock(&mut self, qh: &QueueHandle<Self>) {
        self.relocked = true;
        if let Some(lock) = self.lock.take() {
            lock.unlock_and_destroy();
            log::event("relocked", json!({}));
        }
        self.create_lock(qh);
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
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                log::event("locked", json!({}));
                app.start_lock_timers(conn, qh);
            }
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
