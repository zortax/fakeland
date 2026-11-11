//! The shell side of an applied commit: initial configures, mapping when the first buffer
//! arrives, unmapping when the buffer goes, and bounding box updates of window trees.

use fakeland_trace::Event;
use fakeland_types::WindowId;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::PopupKind;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use super::window_origin;
use crate::server::State;

impl State {
    /// Runs the shell logic of an applied commit.
    pub fn shell_committed(&mut self, surface: &WlSurface) {
        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
            && let Err(e) = popup.send_configure()
        {
            tracing::debug!("popup configure failed: {e}");
        }
        let Some(id) = self.window_of_surface(surface) else {
            return;
        };
        let Some(rec) = self.windows.get(id) else {
            return;
        };
        rec.window.on_commit();
        if rec.toplevel.wl_surface() != surface {
            return;
        }
        let toplevel = rec.toplevel.clone();
        if !toplevel.is_initial_configure_sent() {
            self.send_toplevel_configure(&toplevel);
            return;
        }
        let has_buffer =
            with_renderer_surface_state(surface, |s| s.buffer().is_some()).unwrap_or(false);
        let mapped = rec.mapped_at.is_some();
        match (has_buffer, mapped) {
            (true, false) => self.map_window(id),
            (false, true) => self.unmap_window(id),
            (true, true) => self.place_window(id, false),
            (false, false) => {}
        }
    }

    /// Maps a window into the space and focuses it.
    fn map_window(&mut self, id: WindowId) {
        let now = self.now();
        let Some(rec) = self.windows.get_mut(id) else {
            return;
        };
        rec.mapped_at = Some(now);
        let (window, surface, output) = (rec.window.clone(), rec.surface, rec.output);
        let (app_id, title) = (rec.app_id.clone(), rec.title.clone());
        let wl = rec.toplevel.wl_surface().clone();
        let size = rec.window.geometry().size;
        let origin = self
            .outputs
            .get(output)
            .map(|o| {
                window_origin(
                    rec.placement,
                    o.logical_rect(),
                    rec.floating,
                    fakeland_types::Size::new(size.w, size.h),
                )
            })
            .unwrap_or((rec.floating.x, rec.floating.y));
        self.space.map_element(window, origin, true);
        self.space_dirty = true;
        self.emit(Event::WindowMapped {
            window: id,
            surface,
            app_id,
            title,
            output: Some(output),
        });
        self.send_tree_scale(&wl);
        self.focus_window(Some(id));
    }

    /// Unmaps a window and moves focus to the previous window.
    pub fn unmap_window(&mut self, id: WindowId) {
        let Some(rec) = self.windows.get_mut(id) else {
            return;
        };
        if rec.mapped_at.take().is_none() {
            return;
        }
        let window = rec.window.clone();
        self.space.unmap_elem(&window);
        self.space_dirty = true;
        self.emit(Event::WindowUnmapped { window: id });
        if self.windows.focus == Some(id) {
            let next = self.windows.previous_focus(Some(id));
            self.focus_window(next);
        }
    }
}
