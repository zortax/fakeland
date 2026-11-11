//! Window queries for the session: the public description of each window and matcher lookups.

use fakeland_scenario::WindowRef;
use fakeland_types::{Rect, WindowId};

use super::{MatchError, WindowInfo, WindowMatcher, WindowRecord, placement_rect};
use crate::server::State;

impl State {
    /// Returns the public description of a window.
    pub fn window_info(&self, rec: &WindowRecord) -> WindowInfo {
        let geometry = self
            .space
            .element_geometry(&rec.window)
            .map(|g| Rect::new(g.loc.x, g.loc.y, g.size.w, g.size.h))
            .or_else(|| {
                self.outputs
                    .get(rec.output)
                    .map(|o| placement_rect(rec.placement, o.logical_rect(), rec.floating))
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
            focused: self.windows.focus == Some(rec.id),
            suspended: rec.suspended,
            placement: rec.placement,
        }
    }

    /// Returns every window in mapping order: mapped windows by map time, then unmapped ones.
    pub fn window_infos(&self) -> Vec<WindowInfo> {
        let mut list: Vec<WindowInfo> = self.windows.iter().map(|r| self.window_info(r)).collect();
        list.sort_by_key(|w| (w.mapped_at.is_none(), w.mapped_at, w.id));
        list
    }

    /// Resolves a window reference to a mapped window.
    pub fn resolve_window(&self, r: &WindowRef) -> Result<Option<WindowId>, MatchError> {
        let matcher = WindowMatcher::new(r)?;
        Ok(matcher.select(&self.window_infos()).map(|w| w.id))
    }
}
