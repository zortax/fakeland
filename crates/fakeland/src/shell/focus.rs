//! Toplevel focus and activation. The focused toplevel carries the activated state and sits on
//! top of the space. It has keyboard focus unless a lock or layer surface holds it.

use fakeland_types::WindowId;

use crate::server::State;

impl State {
    /// Focuses a toplevel, or clears toplevel focus with `None`. A toplevel takes keyboard focus
    /// back from a layer surface that took it on demand.
    pub fn focus_window(&mut self, id: Option<WindowId>) {
        if id.is_some() {
            self.layers.focus = None;
        }
        self.set_window_focus(id);
    }

    /// Focuses a toplevel and leaves the focus of layer surfaces as it is.
    pub fn set_window_focus(&mut self, id: Option<WindowId>) {
        let id = id.filter(|w| self.windows.get(*w).is_some());
        let old = self.windows.focus;
        self.windows.focus = id;
        if let Some(w) = id {
            self.windows.push_focus(w);
        }
        self.refresh_keyboard_focus();
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
