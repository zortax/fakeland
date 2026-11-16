//! Lock and unlock. A live client holds the lock until it unlocks. A lock whose client died
//! without unlocking keeps the session locked and yields to the next lock request. A forced
//! unlock ends the lock from the session side.

use fakeland_types::{Nanos, WindowId};
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::session_lock::SessionLocker;

use crate::server::State;

impl State {
    /// Locks the session. The outputs show only lock surfaces from now on, so the lock is
    /// confirmed at once. A second lock fails while a live client holds the session. A lock
    /// left by a dead client yields to the new one.
    pub fn lock_requested(&mut self, locker: SessionLocker) {
        if self.lock.locked {
            if self.lock.held() {
                drop(locker);
                return;
            }
            self.forget_lock_surfaces();
        }
        self.end_popup_grabs();
        self.lock.locked = true;
        self.lock.object = Some(locker.ext_session_lock().clone());
        locker.lock();
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Handles `unlock_and_destroy`. The request may come from a lock object that holds no lock:
    /// a refused one, or the stale one of a forced unlock. The sender dies with the request, so
    /// the decision waits until the dispatch ends.
    pub fn unlock_requested(&mut self) {
        self.schedule_in(Nanos::ZERO, State::settle_unlock);
    }

    /// Unlocks the session when the lock object that holds it is gone. A stale lock object that
    /// died sent the request, and the session stays locked.
    fn settle_unlock(&mut self) {
        if self.lock.stale.as_ref().is_some_and(|s| !s.is_alive()) {
            self.lock.stale = None;
            return;
        }
        let holder_gone = self.lock.object.as_ref().is_none_or(|o| !o.is_alive());
        if self.lock.locked && holder_gone {
            self.unlock_session();
        }
    }

    /// Unlocks the session without the lock client. The client keeps its lock object, and its
    /// later unlock request leaves a newer lock in place. Returns false when the session is not
    /// locked.
    pub fn force_unlock(&mut self) -> bool {
        if !self.lock.locked {
            return false;
        }
        self.lock.stale = self.lock.object.take().filter(|o| o.is_alive());
        self.unlock_session();
        true
    }

    /// Unlocks the session and forgets the lock surfaces.
    fn unlock_session(&mut self) {
        self.lock.locked = false;
        self.lock.object = None;
        self.forget_lock_surfaces();
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Forgets every lock surface.
    fn forget_lock_surfaces(&mut self) {
        let ids: Vec<WindowId> = self.lock.surfaces.iter().map(|s| s.id).collect();
        for id in ids {
            self.forget_lock_surface(id);
        }
        self.lock.focus = None;
    }
}
