//! Toplevel configures: size, bounds and states from the placement, focus and suspension of a
//! window. Each sent configure and each ack lands in the trace.

use fakeland_trace::Event;
use fakeland_types::WindowId;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::xdg::{Configure, ToplevelSurface};

use super::{Placement, placement_rect, window_origin};
use crate::server::State;

impl State {
    /// Sets the pending toplevel state of a window and sends it when the first configure is out.
    pub fn apply_window_state(&mut self, id: WindowId) {
        let Some(rec) = self.windows.get(id) else {
            return;
        };
        if !rec.toplevel.alive() {
            return;
        }
        let Some(out) = self.outputs.get(rec.output) else {
            return;
        };
        let out_rect = out.logical_rect();
        let rect = placement_rect(rec.placement, out_rect, rec.floating);
        // A window takes focus when it maps, so it configures as activated until then.
        let focused = self.windows.focus == Some(id) || rec.mapped_at.is_none();
        let fullscreen_output = (rec.placement == Placement::Fullscreen)
            .then(|| {
                let client = rec.toplevel.wl_surface().client()?;
                out.output.client_outputs(&client).next()
            })
            .flatten();
        let placement = rec.placement;
        let suspended = rec.suspended;
        let toplevel = rec.toplevel.clone();
        toplevel.with_pending_state(|s| {
            s.size = Some((rect.w, rect.h).into());
            s.bounds = Some((out_rect.w, out_rect.h).into());
            toggle(
                &mut s.states,
                XdgState::Maximized,
                placement == Placement::Maximized,
            );
            toggle(
                &mut s.states,
                XdgState::Fullscreen,
                placement == Placement::Fullscreen,
            );
            toggle(&mut s.states, XdgState::Activated, focused);
            toggle(&mut s.states, XdgState::Suspended, suspended);
            s.fullscreen_output = fullscreen_output;
        });
        self.update_surface_feedback(id);
        if toplevel.is_initial_configure_sent() {
            self.send_toplevel_configure(&toplevel);
        }
    }

    /// Sends the pending state of a toplevel when it differs from the last configure.
    pub fn send_toplevel_configure(&mut self, toplevel: &ToplevelSurface) {
        let serial = if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure()
        } else {
            Some(toplevel.send_configure())
        };
        let Some(serial) = serial else { return };
        let Some(surface) = self.surfaces.id_of(toplevel.wl_surface()) else {
            return;
        };
        let (width, height, states) = toplevel.with_pending_state(|s| {
            let size = s.size.unwrap_or_default();
            let states = s
                .states
                .clone()
                .into_iter()
                .filter_map(state_name)
                .map(String::from)
                .collect();
            (size.w, size.h, states)
        });
        self.emit(Event::Configure {
            surface,
            serial: serial.into(),
            width,
            height,
            states,
        });
    }

    /// Records an acked configure. The serial goes on the next commit of the surface.
    pub fn configure_acked(&mut self, surface: &WlSurface, configure: Configure) {
        let serial: u32 = match configure {
            Configure::Toplevel(c) => c.serial.into(),
            Configure::Popup(c) => c.serial.into(),
        };
        let Some(rec) = self.surfaces.record_mut(surface) else {
            return;
        };
        rec.acked_serial = Some(serial);
        let id = rec.id;
        self.emit(Event::AckConfigure {
            surface: id,
            serial,
        });
    }

    /// Re-applies the state of a window and moves it to its placement geometry.
    pub fn relayout_window(&mut self, id: WindowId) {
        self.apply_window_state(id);
        self.place_window(id, false);
    }

    /// Moves a mapped window in the space to its placement geometry.
    pub fn place_window(&mut self, id: WindowId, activate: bool) {
        let Some(rec) = self.windows.get(id) else {
            return;
        };
        if rec.mapped_at.is_none() {
            return;
        }
        let Some(out) = self.outputs.get(rec.output) else {
            return;
        };
        let size = rec.window.geometry().size;
        let origin = window_origin(
            rec.placement,
            out.logical_rect(),
            rec.floating,
            fakeland_types::Size::new(size.w, size.h),
        );
        let window = rec.window.clone();
        if self.space.element_location(&window) != Some(origin.into()) || activate {
            self.space.map_element(window, origin, activate);
            self.space_dirty = true;
        }
    }

    /// Gives a fullscreen window the scanout feedback and other windows the default feedback.
    fn update_surface_feedback(&mut self, id: WindowId) {
        let Some(rec) = self.windows.get(id) else {
            return;
        };
        let feedback = match rec.placement {
            Placement::Fullscreen => self.protocols.scanout_feedback.as_ref(),
            _ => self.protocols.default_feedback.as_ref(),
        };
        let Some(feedback) = feedback else { return };
        smithay::wayland::compositor::with_states(rec.toplevel.wl_surface(), |states| {
            if let Some(state) =
                smithay::wayland::dmabuf::SurfaceDmabufFeedbackState::from_states(states)
            {
                state.set_feedback(feedback);
            }
        });
    }
}

fn toggle(set: &mut smithay::wayland::shell::xdg::ToplevelStateSet, state: XdgState, on: bool) {
    if on {
        set.set(state);
    } else {
        set.unset(state);
    }
}

/// Returns the trace name of a toplevel state.
pub fn state_name(state: XdgState) -> Option<&'static str> {
    Some(match state {
        XdgState::Maximized => "maximized",
        XdgState::Fullscreen => "fullscreen",
        XdgState::Resizing => "resizing",
        XdgState::Activated => "activated",
        XdgState::TiledLeft => "tiled_left",
        XdgState::TiledRight => "tiled_right",
        XdgState::TiledTop => "tiled_top",
        XdgState::TiledBottom => "tiled_bottom",
        XdgState::Suspended => "suspended",
        _ => return None,
    })
}
