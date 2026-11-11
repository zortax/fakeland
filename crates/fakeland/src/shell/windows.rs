//! The window registry: one record per toplevel in creation order, focus history and the cursor.

use std::collections::HashMap;

use fakeland_types::{Logical, OutputId, ProcessId, Rect, SurfaceId, Timestamp, WindowId};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::desktop::Window;
use smithay::input::pointer::CursorImageStatus;
use smithay::utils::{Logical as SLogical, Point};
use smithay::wayland::shell::xdg::ToplevelSurface;

use super::Placement;

/// One toplevel window.
#[derive(Debug, Clone)]
pub struct WindowRecord {
    /// The window id.
    pub id: WindowId,
    /// The smithay window.
    pub window: Window,
    /// The toplevel.
    pub toplevel: ToplevelSurface,
    /// The root surface.
    pub surface: SurfaceId,
    /// The output that shows the window.
    pub output: OutputId,
    /// The placement state.
    pub placement: Placement,
    /// The geometry while floating, in the global space.
    pub floating: Rect<Logical>,
    /// The time of the first buffer, while mapped.
    pub mapped_at: Option<Timestamp>,
    /// The app id.
    pub app_id: String,
    /// The title.
    pub title: String,
    /// The window carries the suspended state.
    pub suspended: bool,
    /// The client pid.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
}

/// The pointer position and image.
#[derive(Debug)]
pub struct CursorState {
    /// The position in the global space.
    pub position: Point<f64, SLogical>,
    /// The image the focused client set.
    pub image: CursorImageStatus,
    /// The built-in image for named cursors.
    pub default: MemoryRenderBuffer,
}

/// Every toplevel window.
#[derive(Debug)]
pub struct Windows {
    list: Vec<WindowRecord>,
    by_surface: HashMap<SurfaceId, WindowId>,
    /// The focused window.
    pub focus: Option<WindowId>,
    history: Vec<WindowId>,
    created: HashMap<OutputId, usize>,
    /// The pointer.
    pub cursor: CursorState,
}

impl Windows {
    /// Builds an empty registry.
    pub fn new(default_cursor: MemoryRenderBuffer) -> Self {
        Self {
            list: Vec::new(),
            by_surface: HashMap::new(),
            focus: None,
            history: Vec::new(),
            created: HashMap::new(),
            cursor: CursorState {
                position: Point::from((0.0, 0.0)),
                image: CursorImageStatus::default_named(),
                default: default_cursor,
            },
        }
    }

    /// Adds a window.
    pub fn insert(&mut self, record: WindowRecord) {
        self.by_surface.insert(record.surface, record.id);
        self.list.push(record);
    }

    /// Removes a window.
    pub fn remove(&mut self, id: WindowId) -> Option<WindowRecord> {
        let i = self.list.iter().position(|w| w.id == id)?;
        let rec = self.list.remove(i);
        self.by_surface.remove(&rec.surface);
        self.history.retain(|w| *w != id);
        if self.focus == Some(id) {
            self.focus = None;
        }
        Some(rec)
    }

    /// Returns a window.
    pub fn get(&self, id: WindowId) -> Option<&WindowRecord> {
        self.list.iter().find(|w| w.id == id)
    }

    /// Returns a window for changes.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowRecord> {
        self.list.iter_mut().find(|w| w.id == id)
    }

    /// Returns the window of a root surface.
    pub fn by_surface(&self, surface: SurfaceId) -> Option<WindowId> {
        self.by_surface.get(&surface).copied()
    }

    /// Returns every window in creation order.
    pub fn iter(&self) -> impl Iterator<Item = &WindowRecord> {
        self.list.iter()
    }

    /// Returns the ids of the windows on an output.
    pub fn on_output(&self, output: OutputId) -> Vec<WindowId> {
        self.list
            .iter()
            .filter(|w| w.output == output)
            .map(|w| w.id)
            .collect()
    }

    /// Returns the cascade index of the next window on an output.
    pub fn next_index(&mut self, output: OutputId) -> usize {
        let n = self.created.entry(output).or_insert(0);
        let i = *n;
        *n += 1;
        i
    }

    /// Moves a window to the top of the focus history.
    pub fn push_focus(&mut self, id: WindowId) {
        self.history.retain(|w| *w != id);
        self.history.push(id);
    }

    /// Returns the most recently focused mapped window other than `except`.
    pub fn previous_focus(&self, except: Option<WindowId>) -> Option<WindowId> {
        self.history
            .iter()
            .rev()
            .copied()
            .filter(|w| Some(*w) != except)
            .find(|w| self.get(*w).is_some_and(|r| r.mapped_at.is_some()))
    }
}
