//! Surface bookkeeping: the session id of every live surface, its client, the output whose
//! scheduler takes its commits, and the commits that wait for smithay to apply them.

use std::collections::{HashMap, VecDeque};

use fakeland_types::{CommitId, OutputId, SurfaceId, Timestamp};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::backend::ObjectId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

/// One commit that smithay has not applied yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyEntry {
    /// The commit.
    pub commit: CommitId,
    /// The commit attached a new buffer.
    pub new_buffer: bool,
}

/// One live surface.
#[derive(Debug)]
pub struct SurfaceRecord {
    /// The session id.
    pub id: SurfaceId,
    /// The wayland surface.
    pub wl: WlSurface,
    /// The output whose scheduler takes the commits.
    pub output: Option<OutputId>,
    /// Commits waiting for smithay to apply them, oldest first.
    pub applies: VecDeque<ApplyEntry>,
    /// The commit time of the last commit with damage.
    pub last_damage: Option<Timestamp>,
    /// The configure serial acked since the last commit.
    pub acked_serial: Option<u32>,
    /// The commit count.
    pub commits: u64,
}

/// Every live surface.
#[derive(Debug, Default)]
pub struct Surfaces {
    by_id: HashMap<SurfaceId, SurfaceRecord>,
    by_object: HashMap<ObjectId, SurfaceId>,
}

impl Surfaces {
    /// Adds a surface.
    pub fn insert(&mut self, record: SurfaceRecord) {
        self.by_object.insert(record.wl.id(), record.id);
        self.by_id.insert(record.id, record);
    }

    /// Removes a surface.
    pub fn remove(&mut self, surface: &WlSurface) -> Option<SurfaceRecord> {
        let id = self.by_object.remove(&surface.id())?;
        self.by_id.remove(&id)
    }

    /// Returns the session id of a surface.
    pub fn id_of(&self, surface: &WlSurface) -> Option<SurfaceId> {
        self.by_object.get(&surface.id()).copied()
    }

    /// Returns a surface.
    pub fn get(&self, id: SurfaceId) -> Option<&SurfaceRecord> {
        self.by_id.get(&id)
    }

    /// Returns a surface for changes.
    pub fn get_mut(&mut self, id: SurfaceId) -> Option<&mut SurfaceRecord> {
        self.by_id.get_mut(&id)
    }

    /// Returns the record of a wayland surface.
    pub fn record_mut(&mut self, surface: &WlSurface) -> Option<&mut SurfaceRecord> {
        let id = self.id_of(surface)?;
        self.by_id.get_mut(&id)
    }

    /// Returns every live surface.
    pub fn iter(&self) -> impl Iterator<Item = &SurfaceRecord> {
        self.by_id.values()
    }

    /// Takes the commits that one smithay apply covers. A synchronized subtree applies every
    /// queued commit at once; a desynchronized surface applies its oldest commit.
    pub fn take_applied(&mut self, id: SurfaceId, all: bool) -> Vec<ApplyEntry> {
        let Some(rec) = self.by_id.get_mut(&id) else {
            return Vec::new();
        };
        if all {
            rec.applies.drain(..).collect()
        } else {
            rec.applies.pop_front().into_iter().collect()
        }
    }
}
