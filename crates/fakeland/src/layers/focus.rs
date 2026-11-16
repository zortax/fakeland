//! Keyboard focus across shells. A lock surface holds focus while the session is locked. Else
//! the topmost exclusive layer surface on the top or overlay layer holds it. Else a layer surface
//! that took focus on demand holds it, else the focused toplevel. A press moves focus to the
//! toplevel or the interactive layer surface under the pointer, and a press elsewhere ends the
//! focus that a layer surface took on demand.

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
    /// layer surface or the lock surface under it. A press anywhere else ends the focus that a
    /// layer surface took on demand.
    pub fn focus_at(&mut self, point: Point<f64, Logical>) {
        let hit = self.hit(point).map(|h| h.window);
        if self.lock.locked {
            if hit.is_some() {
                self.lock.focus = hit;
                self.refresh_keyboard_focus();
            }
            return;
        }
        match hit {
            Some(id) if self.windows.get(id).is_some() => {
                if self.windows.focus != Some(id) || self.layers.focus.is_some() {
                    self.focus_window(Some(id));
                }
            }
            Some(id) if self.focus_layer(id) => {}
            _ => {
                if self.layers.focus.take().is_some() {
                    self.refresh_keyboard_focus();
                }
            }
        }
    }
}
