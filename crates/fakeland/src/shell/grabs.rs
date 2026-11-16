//! Popup grabs. A grabbed popup chain holds keyboard focus, and pointer focus stays on its
//! client. A press on a surface of another client, or on no surface, dismisses the chain with
//! `popup_done`, and the press then reaches the surface under the pointer. A lock and a focus
//! operation end the grab too.

use smithay::desktop::{
    PopupKeyboardGrab, PopupKind, PopupManager, PopupPointerGrab, PopupUngrabStrategy,
    find_popup_root_surface,
};
use smithay::input::Seat;
use smithay::input::pointer::Focus;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Serial};
use smithay::wayland::shell::xdg::PopupSurface;

use crate::server::State;

impl State {
    /// Handles `xdg_popup.grab`. The popup chain takes the keyboard and the pointer. A grab
    /// dismisses the popup at once while the session is locked or while an exclusive layer
    /// surface outside the chain holds the keyboard.
    pub fn popup_grab_requested(&mut self, popup: PopupSurface, seat: WlSeat, serial: Serial) {
        let kind = PopupKind::Xdg(popup);
        let Ok(root) = find_popup_root_surface(&kind) else {
            return;
        };
        let seat = Seat::<State>::from_resource(&seat);
        let root_known = self.window_of_surface(&root).is_some();
        let blocked = self.lock.locked || self.exclusive_layer().is_some_and(|s| s != root);
        let Some(seat) = seat.filter(|_| root_known && !blocked) else {
            let _ = PopupManager::dismiss_popup(&root, &kind);
            return;
        };
        let mut grab = match self.popups.grab_popup(root, kind, &seat, serial) {
            Ok(grab) => grab,
            Err(e) => {
                tracing::debug!("popup grab refused: {e:?}");
                return;
            }
        };
        let previous = grab.previous_serial().unwrap_or_else(|| grab.serial());
        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed() && !(keyboard.has_grab(serial) || keyboard.has_grab(previous))
            {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed() && !(pointer.has_grab(serial) || pointer.has_grab(previous)) {
                grab.ungrab(PopupUngrabStrategy::All);
                if let Some(keyboard) = seat.get_keyboard() {
                    keyboard.unset_grab(self);
                }
                self.refresh_keyboard_focus();
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
        self.popup_grab = Some(grab);
    }

    /// Dismisses the grabbed popup chain and releases the keyboard and the pointer.
    pub fn end_popup_grabs(&mut self) {
        let Some(mut grab) = self.popup_grab.take() else {
            return;
        };
        if !grab.has_ended() {
            grab.ungrab(PopupUngrabStrategy::All);
        }
        let serials: Vec<Serial> = [Some(grab.serial()), grab.previous_serial()]
            .into_iter()
            .flatten()
            .collect();
        if let Some(pointer) = self.seat.get_pointer()
            && serials.iter().any(|s| pointer.has_grab(*s))
        {
            pointer.unset_grab(
                self,
                SERIAL_COUNTER.next_serial(),
                self.now().wayland_millis(),
            );
        }
        if let Some(keyboard) = self.seat.get_keyboard()
            && serials.iter().any(|s| keyboard.has_grab(*s))
        {
            keyboard.unset_grab(self);
        }
        self.refresh_keyboard_focus();
    }

    /// Forgets a popup grab whose popups are gone, so keyboard focus follows the focus rules
    /// again.
    pub fn reap_popup_grab(&mut self) {
        if self.popup_grab.as_ref().is_some_and(|g| g.has_ended()) {
            self.end_popup_grabs();
        }
    }

    /// Ends the popup grab for a touch down at a global point outside the client of the grab.
    pub fn popup_grab_touch(&mut self, point: Point<f64, Logical>) {
        let Some(grabbed) = self.popup_grab.as_ref().and_then(|g| g.current_grab()) else {
            return;
        };
        let inside = self
            .surface_under(point)
            .is_some_and(|(s, _)| s.client() == grabbed.client());
        if !inside {
            self.end_popup_grabs();
        }
    }
}
