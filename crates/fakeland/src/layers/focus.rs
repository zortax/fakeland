//! Keyboard focus across shells. A lock surface holds focus while the session is locked. Else
//! the topmost exclusive layer surface on the top or overlay layer holds it. Else a layer surface
//! that took focus on demand holds it, else the focused toplevel. A press moves focus to the
//! toplevel or the interactive layer surface under the pointer.

use fakeland_types::WindowId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer};

use crate::server::State;

impl State {
    /// Returns the surface that holds keyboard focus under the focus rules.
    fn keyboard_target(&self) -> Option<WlSurface> {
        if self.lock.locked {
            return self.lock_focus_surface();
        }
        if let Some(surface) = self.exclusive_layer() {
            return Some(surface);
        }
        let on_demand = self
            .layers
            .focus
            .and_then(|id| self.layers.get(id))
            .filter(|l| l.mapped_at.is_some() && l.layer.can_receive_keyboard_focus());
        if let Some(rec) = on_demand {
            return Some(rec.layer.wl_surface().clone());
        }
        self.windows
            .focus
            .and_then(|w| self.windows.get(w))
            .map(|r| r.toplevel.wl_surface().clone())
    }

    /// Returns the topmost mapped exclusive layer surface on the top or overlay layer.
    fn exclusive_layer(&self) -> Option<WlSurface> {
        self.layers
            .iter()
            .rev()
            .filter(|l| l.mapped_at.is_some())
            .filter_map(|l| {
                let state = l.layer.cached_state();
                let rank = match state.layer {
                    Layer::Overlay => 1,
                    Layer::Top => 0,
                    _ => return None,
                };
                (state.keyboard_interactivity == KeyboardInteractivity::Exclusive)
                    .then(|| (rank, l.layer.wl_surface().clone()))
            })
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, surface)| surface)
    }

    /// Gives keyboard focus to the surface that the focus rules select.
    pub fn refresh_keyboard_focus(&mut self) {
        let target = self.keyboard_target();
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };
        if keyboard.current_focus() != target {
            keyboard.set_focus(self, target, SERIAL_COUNTER.next_serial());
        }
    }

    /// Returns the window, layer surface or lock surface with keyboard focus.
    pub fn keyboard_focus_window(&self) -> Option<WindowId> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        self.window_of_surface(&focus)
    }

    /// Gives keyboard focus to a layer surface on demand. Returns false when the surface takes
    /// no keyboard focus.
    pub fn focus_layer(&mut self, id: WindowId) -> bool {
        let Some(rec) = self.layers.get(id) else {
            return false;
        };
        if !rec.layer.can_receive_keyboard_focus() {
            return false;
        }
        self.layers.focus = Some(id);
        self.refresh_keyboard_focus();
        true
    }

    /// Moves keyboard focus for a press at a global point: to the toplevel, the interactive
    /// layer surface or the lock surface under it. A press elsewhere keeps the focus.
    pub fn focus_at(&mut self, point: Point<f64, Logical>) {
        let Some(hit) = self.hit(point) else {
            return;
        };
        let id = hit.window;
        if self.lock.locked {
            self.lock.focus = Some(id);
            self.refresh_keyboard_focus();
        } else if self.layers.get(id).is_some() {
            self.focus_layer(id);
        } else if self.windows.focus != Some(id) || self.layers.focus.is_some() {
            self.focus_window(Some(id));
        }
    }
}
