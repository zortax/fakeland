//! The lock state, the lock object that holds the session, and the lock surface registry.

use fakeland_types::{OutputId, ProcessId, SurfaceId, Timestamp, WindowId};
use smithay::backend::renderer::element::Id;
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::session_lock::LockSurface;

/// One lock surface.
#[derive(Debug, Clone)]
pub struct LockRecord {
    /// The window id.
    pub id: WindowId,
    /// The smithay lock surface.
    pub surface: LockSurface,
    /// The root surface.
    pub sid: SurfaceId,
    /// The output that the surface covers.
    pub output: OutputId,
    /// The time of the first buffer, while mapped.
    pub mapped_at: Option<Timestamp>,
    /// The client pid.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
}

/// The session lock.
#[derive(Debug)]
pub struct SessionLock {
    /// The session is locked: only lock surfaces show and receive input.
    pub locked: bool,
    /// The lock object that holds the session while it is locked.
    pub object: Option<ExtSessionLockV1>,
    /// The lock object of a forced unlock. Its client still sees the session locked, and its
    /// unlock request leaves a newer lock in place.
    pub stale: Option<ExtSessionLockV1>,
    /// The lock surfaces in creation order.
    pub surfaces: Vec<LockRecord>,
    /// The lock surface that a press focused.
    pub focus: Option<WindowId>,
    /// The render element id of the black backdrop behind the lock surfaces.
    pub backdrop: Id,
}

impl Default for SessionLock {
    fn default() -> Self {
        Self {
            locked: false,
            object: None,
            stale: None,
            surfaces: Vec::new(),
            focus: None,
            backdrop: Id::new(),
        }
    }
}

impl SessionLock {
    /// Returns a lock surface.
    pub fn get(&self, id: WindowId) -> Option<&LockRecord> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    /// Returns the lock surface of a root surface.
    pub fn by_surface(&self, sid: SurfaceId) -> Option<WindowId> {
        self.surfaces.iter().find(|s| s.sid == sid).map(|s| s.id)
    }

    /// Returns true while a live client holds the lock: the lock object or a lock surface is
    /// alive.
    pub fn held(&self) -> bool {
        self.object.as_ref().is_some_and(|o| o.is_alive())
            || self.surfaces.iter().any(|s| s.surface.alive())
    }

    /// Removes a lock surface.
    pub fn remove(&mut self, id: WindowId) -> Option<LockRecord> {
        let i = self.surfaces.iter().position(|s| s.id == id)?;
        if self.focus == Some(id) {
            self.focus = None;
        }
        Some(self.surfaces.remove(i))
    }
}
