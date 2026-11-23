//! Window queries for the session: the public description of each toplevel, layer surface and
//! lock surface, and matcher lookups.

use fakeland_scenario::WindowRef;
use fakeland_types::{Logical, Rect, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use super::{MatchError, Placement, WindowInfo, WindowKind, WindowMatcher, WindowRecord};
use crate::layers::{LayerRecord, layer_info};
use crate::lock::LockRecord;
use crate::server::State;

impl State {
    /// Returns the public description of a toplevel.
    pub fn window_info(&self, rec: &WindowRecord) -> WindowInfo {
        let geometry = self
            .space
            .element_geometry(&rec.window)
            .map(|g| Rect::new(g.loc.x, g.loc.y, g.size.w, g.size.h))
            .or_else(|| {
                self.outputs
                    .get(rec.output)
                    .map(|_| self.placement_geometry(rec))
            })
            .unwrap_or(rec.floating);
        WindowInfo {
            id: rec.id,
            app_id: rec.app_id.clone(),
            title: rec.title.clone(),
            pid: rec.pid,
            app: rec.app,
            output: rec.output,
            geometry,
            surface: rec.surface,
            mapped_at: rec.mapped_at,
            focused: self.windows.focus == Some(rec.id)
                && self.keyboard_focus_window() == Some(rec.id),
            suspended: rec.suspended,
            placement: rec.placement,
            kind: WindowKind::Toplevel,
            layer: None,
        }
    }

    /// Returns the public description of a layer surface.
    pub fn layer_window_info(&self, rec: &LayerRecord) -> WindowInfo {
        let origin = self
            .layer_origin(rec.id)
            .map(|p| (p.x, p.y))
            .or_else(|| {
                self.outputs
                    .get(rec.output)
                    .map(|o| (o.position.x, o.position.y))
            })
            .unwrap_or_default();
        let configured = rec
            .layer
            .layer_surface()
            .with_pending_state(|s| s.size)
            .unwrap_or_default();
        let (w, h) = surface_size(rec.layer.wl_surface()).unwrap_or((configured.w, configured.h));
        WindowInfo {
            id: rec.id,
            app_id: String::new(),
            title: String::new(),
            pid: rec.pid,
            app: rec.app,
            output: rec.output,
            geometry: Rect::new(origin.0, origin.1, w, h),
            surface: rec.surface,
            mapped_at: rec.mapped_at,
            focused: self.keyboard_focus_window() == Some(rec.id),
            suspended: false,
            placement: Placement::Floating,
            kind: WindowKind::Layer,
            layer: Some(layer_info(&rec.layer)),
        }
    }

    /// Returns the public description of a lock surface.
    pub fn lock_window_info(&self, rec: &LockRecord) -> WindowInfo {
        let geometry = self
            .outputs
            .get(rec.output)
            .map(|o| o.logical_rect())
            .unwrap_or_default();
        WindowInfo {
            id: rec.id,
            app_id: String::new(),
            title: String::new(),
            pid: rec.pid,
            app: rec.app,
            output: rec.output,
            geometry,
            surface: rec.sid,
            mapped_at: rec.mapped_at,
            focused: self.keyboard_focus_window() == Some(rec.id),
            suspended: false,
            placement: Placement::Fullscreen,
            kind: WindowKind::Lock,
            layer: None,
        }
    }

    /// Returns the public description of a toplevel, layer surface or lock surface.
    pub fn info_of(&self, id: WindowId) -> Option<WindowInfo> {
        if let Some(rec) = self.windows.get(id) {
            return Some(self.window_info(rec));
        }
        if let Some(rec) = self.layers.get(id) {
            return Some(self.layer_window_info(rec));
        }
        self.lock.get(id).map(|rec| self.lock_window_info(rec))
    }

    /// Returns the output of a toplevel, layer surface or lock surface.
    pub fn window_output(&self, id: WindowId) -> Option<fakeland_types::OutputId> {
        self.windows
            .get(id)
            .map(|w| w.output)
            .or_else(|| self.layers.get(id).map(|l| l.output))
            .or_else(|| self.lock.get(id).map(|l| l.output))
    }

    /// Returns every toplevel, layer surface and lock surface in mapping order: mapped ones by
    /// map time, then unmapped ones.
    pub fn window_infos(&self) -> Vec<WindowInfo> {
        let mut list: Vec<WindowInfo> = self.windows.iter().map(|r| self.window_info(r)).collect();
        list.extend(self.layers.iter().map(|r| self.layer_window_info(r)));
        list.extend(self.lock.surfaces.iter().map(|r| self.lock_window_info(r)));
        list.sort_by_key(|w| (w.mapped_at.is_none(), w.mapped_at, w.id));
        list
    }

    /// Resolves a window reference to a mapped window.
    pub fn resolve_window(&self, r: &WindowRef) -> Result<Option<WindowId>, MatchError> {
        let matcher = WindowMatcher::new(r)?;
        Ok(matcher.select(&self.window_infos()).map(|w| w.id))
    }

    /// Returns the geometry of the placement of an unmapped toplevel.
    fn placement_geometry(&self, rec: &WindowRecord) -> Rect<Logical> {
        self.placement_area(rec.output, rec.placement)
            .map(|area| super::placement_rect(rec.placement, area, rec.floating))
            .unwrap_or(rec.floating)
    }
}

/// Returns the logical size of the buffer of a surface.
fn surface_size(surface: &WlSurface) -> Option<(i32, i32)> {
    with_renderer_surface_state(surface, |s| s.surface_size())
        .flatten()
        .map(|s| (s.w, s.h))
}
