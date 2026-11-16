//! Lock and unlock. A live client holds the lock until it unlocks. A lock whose client died
//! without unlocking keeps the session locked and yields to the next lock request. A forced
//! unlock ends the lock from the session side.

use fakeland_types::WindowId;
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
            self.reset_locked_outputs();
        }
        self.end_popup_grabs();
        self.lock.locked = true;
        self.lock.object = Some(locker.ext_session_lock().clone());
        locker.lock();
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Handles `unlock_and_destroy`. The session unlocks when the sender holds the lock. A
    /// refused lock object and the lock object of a forced unlock leave the session as it is.
    pub fn unlock_requested(&mut self) {
        if self.lock.holder_sent() {
            self.unlock_session();
        }
    }

    /// Unlocks the session without the lock client. Returns false when the session is not
    /// locked. The client keeps its lock object and sees the session locked. Its later unlock
    /// request leaves a newer lock in place, and its next lock may cover the same outputs.
    pub fn force_unlock(&mut self) -> bool {
        if !self.lock.locked {
            return false;
        }
        self.unlock_session();
        self.reset_locked_outputs();
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
