//! The overlays of a capture of the space: the layer surfaces above and below the toplevels, or
//! the lock surfaces alone while the session is locked.

use crate::render::{LockScene, Overlays, SceneSurface};
use crate::server::State;
use crate::shell::ShellLayer;

impl State {
    /// Returns the layer and lock surfaces of a capture of the space, topmost first.
    pub fn overlays(&self) -> Overlays {
        if self.lock.locked {
            let surfaces = self
                .lock
                .surfaces
                .iter()
                .filter(|s| s.mapped_at.is_some())
                .filter_map(|s| {
                    Some(SceneSurface {
                        surface: s.surface.wl_surface().clone(),
                        origin: self.lock_origin(s.id)?,
                    })
                })
                .collect();
            return Overlays {
                lock: Some(LockScene {
                    surfaces,
                    backdrop: self.lock.backdrop.clone(),
                }),
                ..Overlays::default()
            };
        }
        let mut overlays = Overlays::default();
        for l in self.all_layers().into_iter().rev() {
            let item = SceneSurface {
                surface: l.layer,
                origin: l.origin,
            };
            if l.level >= ShellLayer::Top {
                overlays.above.push(item);
            } else {
                overlays.below.push(item);
            }
        }
        overlays
    }
}
