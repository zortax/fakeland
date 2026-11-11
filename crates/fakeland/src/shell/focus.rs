//! Keyboard focus and activation. The focused window carries the activated state, sits on top of
//! the space and has keyboard focus.

use fakeland_types::WindowId;
use smithay::utils::SERIAL_COUNTER;

use crate::server::State;

impl State {
    /// Focuses a window, or clears focus with `None`.
    pub fn focus_window(&mut self, id: Option<WindowId>) {
        let id = id.filter(|w| self.windows.get(*w).is_some());
        let old = self.windows.focus;
        self.windows.focus = id;
        if let Some(w) = id {
            self.windows.push_focus(w);
        }
        let surface = id
            .and_then(|w| self.windows.get(w))
            .map(|r| r.toplevel.wl_surface().clone());
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
        }
        if let Some(rec) = id.and_then(|w| self.windows.get(w))
            && rec.mapped_at.is_some()
        {
            let window = rec.window.clone();
            self.space.raise_element(&window, true);
            self.space_dirty = true;
        }
        if old != id {
            for w in [old, id].into_iter().flatten() {
                self.apply_window_state(w);
            }
        }
    }
}
