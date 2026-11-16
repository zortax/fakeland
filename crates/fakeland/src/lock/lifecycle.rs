//! Lock and unlock, lock surfaces that cover their output, their mapping on commit, and the hit
//! test and keyboard focus target while the session is locked.

use fakeland_trace::Event;
use fakeland_types::{OutputId, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::utils::under_from_surface_tree;
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::session_lock::{LockSurface, SessionLocker};

use super::LockRecord;
use crate::layers::Hit;
use crate::server::{ClientState, State};

impl State {
    /// Locks the session. The outputs show only lock surfaces from now on, so the lock is
    /// confirmed at once. A second lock fails while the session is locked.
    pub fn lock_requested(&mut self, locker: SessionLocker) {
        if self.lock.locked {
            drop(locker);
            return;
        }
        self.lock.locked = true;
        locker.lock();
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Unlocks the session and forgets the lock surfaces.
    pub fn unlock_requested(&mut self) {
        self.lock.locked = false;
        let ids: Vec<WindowId> = self.lock.surfaces.iter().map(|s| s.id).collect();
        for id in ids {
            self.forget_lock_surface(id);
        }
        self.lock.focus = None;
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Registers a lock surface and sizes it to its output.
    pub fn lock_surface_created(&mut self, surface: LockSurface, output: WlOutput) {
        let wl = surface.wl_surface().clone();
        let Some(sid) = self.surfaces.id_of(&wl) else {
            return;
        };
        let Some(output) = Output::from_resource(&output).and_then(|o| self.outputs.id_of(&o))
        else {
            return;
        };
        let Some(out) = self.outputs.get(output) else {
            return;
        };
        let rect = out.logical_rect();
        surface.with_pending_state(|s| {
            s.size = Some((rect.w.max(1) as u32, rect.h.max(1) as u32).into());
        });
        out.output.enter(&wl);
        let pid = wl
            .client()
            .and_then(|c| c.get_data::<ClientState>().and_then(|d| d.pid));
        let id = self.ids.window();
        self.lock.surfaces.push(LockRecord {
            id,
            surface,
            sid,
            output,
            mapped_at: None,
            pid,
            app: pid.and_then(|p| self.processes.root_of(p)),
        });
        self.surface_role(&wl, "ext_session_lock_surface_v1", None);
        self.send_tree_scale(&wl);
    }

    /// Runs the shell logic of an applied commit of a lock surface. Returns false when the
    /// surface is no lock surface.
    pub fn lock_committed(&mut self, surface: &WlSurface) -> bool {
        let Some(id) = self
            .surfaces
            .id_of(surface)
            .and_then(|s| self.lock.by_surface(s))
        else {
            return false;
        };
        let has_buffer =
            with_renderer_surface_state(surface, |s| s.buffer().is_some()).unwrap_or(false);
        let now = self.now();
        let Some(rec) = self.lock.surfaces.iter_mut().find(|s| s.id == id) else {
            return true;
        };
        match (has_buffer, rec.mapped_at.is_some()) {
            (true, false) => {
                rec.mapped_at = Some(now);
                let (sid, output) = (rec.sid, rec.output);
                self.emit(Event::WindowMapped {
                    window: id,
                    surface: sid,
                    app_id: String::new(),
                    title: String::new(),
                    output: Some(output),
                });
                self.send_tree_scale(surface);
            }
            (false, true) => {
                rec.mapped_at = None;
                self.emit(Event::WindowUnmapped { window: id });
            }
            _ => {}
        }
        self.refresh_keyboard_focus();
        self.space_dirty = true;
        true
    }

    /// Drops a lock surface.
    pub fn forget_lock_surface(&mut self, id: WindowId) {
        let Some(rec) = self.lock.remove(id) else {
            return;
        };
        if rec.mapped_at.is_some() {
            self.emit(Event::WindowUnmapped { window: id });
        }
        self.space_dirty = true;
    }

    /// Drops the lock surfaces of an output that goes away.
    pub fn close_output_locks(&mut self, output: OutputId) {
        let ids: Vec<WindowId> = self
            .lock
            .surfaces
            .iter()
            .filter(|s| s.output == output)
            .map(|s| s.id)
            .collect();
        for id in ids {
            self.forget_lock_surface(id);
        }
        self.refresh_keyboard_focus();
    }

    /// Sends the new size of an output to its lock surfaces.
    pub fn resize_output_locks(&mut self, output: OutputId) {
        let Some(rect) = self.outputs.get(output).map(|o| o.logical_rect()) else {
            return;
        };
        for rec in self.lock.surfaces.iter().filter(|s| s.output == output) {
            rec.surface.with_pending_state(|s| {
                s.size = Some((rect.w.max(1) as u32, rect.h.max(1) as u32).into());
            });
            rec.surface.send_configure();
        }
    }

    /// Returns the lock surface that holds keyboard focus while the session is locked: the one a
    /// press focused, else the one on the output under the pointer, else the first mapped one.
    pub fn lock_focus_surface(&self) -> Option<WlSurface> {
        let mapped = |id: WindowId| self.lock.get(id).filter(|r| r.mapped_at.is_some());
        let under = self.output_at(self.windows.cursor.position);
        self.lock
            .focus
            .and_then(mapped)
            .or_else(|| {
                self.lock
                    .surfaces
                    .iter()
                    .find(|s| s.mapped_at.is_some() && Some(s.output) == under)
            })
            .or_else(|| self.lock.surfaces.iter().find(|s| s.mapped_at.is_some()))
            .map(|r| r.surface.wl_surface().clone())
    }

    /// Returns the origin of a lock surface in the global logical space.
    pub fn lock_origin(&self, id: WindowId) -> Option<Point<i32, Logical>> {
        let rec = self.lock.get(id)?;
        let rect = self.outputs.get(rec.output)?.logical_rect();
        Some(Point::from((rect.x, rect.y)))
    }

    /// Returns the lock surface under a global point.
    pub fn lock_hit(&self, point: Point<f64, Logical>) -> Option<Hit> {
        let output = self.output_at(point)?;
        let rec = self
            .lock
            .surfaces
            .iter()
            .find(|s| s.output == output && s.mapped_at.is_some())?;
        let origin = self.lock_origin(rec.id)?;
        let (surface, at) = under_from_surface_tree(
            rec.surface.wl_surface(),
            point - origin.to_f64(),
            (0, 0),
            WindowSurfaceType::ALL,
        )?;
        Some(Hit {
            window: rec.id,
            surface,
            origin: (at + origin).to_f64(),
        })
    }
}
