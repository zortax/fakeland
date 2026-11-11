//! Toplevel lifecycle: creation with the policy layout, identity changes, client requests for
//! maximize and fullscreen, and destruction.

use fakeland_trace::Event;
use fakeland_types::{OutputId, ProcessId, Rect, WindowId};
use smithay::desktop::Window;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};

use super::{Placement, WindowRecord, initial_layout};
use crate::server::{ClientState, State};

impl State {
    /// Registers a new toplevel and prepares its first configure.
    pub fn toplevel_created(&mut self, toplevel: ToplevelSurface) {
        let surface = toplevel.wl_surface().clone();
        let Some(sid) = self.surfaces.id_of(&surface) else {
            return;
        };
        let pid = surface
            .client()
            .and_then(|c| c.get_data::<ClientState>().and_then(|d| d.pid));
        let output = self.output_for_new_window(pid);
        let index = self.windows.next_index(output);
        let out_rect = self
            .outputs
            .get(output)
            .map(|o| o.logical_rect())
            .unwrap_or(Rect::new(0, 0, 1920, 1080));
        let layout = initial_layout(self.config.window_policy, out_rect, index);
        let floating = match layout.placement {
            Placement::Floating => layout.rect,
            _ => super::cascade(out_rect, fakeland_types::Size::new(800, 600), index),
        };
        let (app_id, title) = toplevel_identity(&toplevel);
        let id = self.ids.window();
        self.windows.insert(WindowRecord {
            id,
            window: Window::new_wayland_window(toplevel.clone()),
            toplevel: toplevel.clone(),
            surface: sid,
            output,
            placement: layout.placement,
            floating,
            mapped_at: None,
            app_id,
            title,
            suspended: false,
            pid,
            app: pid.and_then(|p| self.processes.root_of(p)),
        });
        let parent = toplevel.parent();
        self.surface_role(&surface, "xdg_toplevel", parent.as_ref());
        self.apply_window_state(id);
    }

    /// Returns the output of a new window: the launch output of its process, or the primary
    /// output.
    fn output_for_new_window(&self, pid: Option<ProcessId>) -> OutputId {
        pid.and_then(|p| self.processes.output_for(p))
            .and_then(|o| self.outputs.resolve(Some(o)))
            .or_else(|| self.outputs.primary())
            .unwrap_or(OutputId(0))
    }

    /// Handles a title or app id change.
    pub fn toplevel_identity_changed(&mut self, toplevel: &ToplevelSurface) {
        let Some(id) = self.window_of_surface(toplevel.wl_surface()) else {
            return;
        };
        let (app_id, title) = toplevel_identity(toplevel);
        let Some(rec) = self.windows.get_mut(id) else {
            return;
        };
        let mapped = rec.mapped_at.is_some();
        let title_changed = rec.title != title;
        let app_id_changed = rec.app_id != app_id;
        rec.title = title.clone();
        rec.app_id = app_id.clone();
        if !mapped {
            return;
        }
        if title_changed {
            self.emit(Event::WindowTitle { window: id, title });
        }
        if app_id_changed {
            self.emit(Event::WindowAppId { window: id, app_id });
        }
    }

    /// Handles `set_maximized` and `unset_maximized`.
    pub fn toplevel_maximize_request(&mut self, toplevel: &ToplevelSurface, on: bool) {
        let Some(id) = self.window_of_surface(toplevel.wl_surface()) else {
            return;
        };
        if let Some(rec) = self.windows.get_mut(id) {
            rec.placement = if on {
                Placement::Maximized
            } else {
                Placement::Floating
            };
        }
        self.relayout_window(id);
    }

    /// Handles `set_fullscreen` and `unset_fullscreen`.
    pub fn toplevel_fullscreen_request(
        &mut self,
        toplevel: &ToplevelSurface,
        on: bool,
        output: Option<WlOutput>,
    ) {
        let Some(id) = self.window_of_surface(toplevel.wl_surface()) else {
            return;
        };
        let target = output
            .as_ref()
            .and_then(smithay::output::Output::from_resource)
            .and_then(|o| self.outputs.id_of(&o));
        let fallback = match self.config.window_policy {
            crate::config::WindowPolicy::Floating { .. } => Placement::Floating,
            _ => Placement::Maximized,
        };
        if let Some(rec) = self.windows.get_mut(id) {
            rec.placement = if on { Placement::Fullscreen } else { fallback };
            if let Some(o) = target {
                rec.output = o;
            }
        }
        self.relayout_window(id);
    }

    /// Forgets a destroyed toplevel.
    pub fn toplevel_gone(&mut self, toplevel: &ToplevelSurface) {
        let Some(id) = self
            .windows
            .iter()
            .find(|w| w.toplevel == *toplevel)
            .map(|w| w.id)
        else {
            return;
        };
        self.forget_window(id);
    }

    /// Unmaps and drops a window.
    pub fn forget_window(&mut self, id: WindowId) {
        self.unmap_window(id);
        self.windows.remove(id);
        self.space_dirty = true;
    }

    /// Returns the window of any surface in a window tree: subsurfaces and popups included.
    pub fn window_of_surface(
        &self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) -> Option<WindowId> {
        if !surface.is_alive() {
            return None;
        }
        let mut root = surface.clone();
        while let Some(parent) = smithay::wayland::compositor::get_parent(&root) {
            root = parent;
        }
        if let Some(id) = self
            .surfaces
            .id_of(&root)
            .and_then(|s| self.windows.by_surface(s))
        {
            return Some(id);
        }
        let popup = self.popups.find_popup(&root)?;
        let top = smithay::desktop::find_popup_root_surface(&popup).ok()?;
        if top.id() == root.id() {
            return None;
        }
        self.window_of_surface(&top)
    }
}

/// Returns the app id and title of a toplevel.
pub fn toplevel_identity(toplevel: &ToplevelSurface) -> (String, String) {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|d| d.lock().ok())
            .map(|d| {
                (
                    d.app_id.clone().unwrap_or_default(),
                    d.title.clone().unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    })
}
