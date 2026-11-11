//! The set of surfaces that have a live marks object. The protocol allows one marks object per
//! surface, so the crate refuses a second one before the compositor raises an error.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use wayland_client::backend::ObjectId;

/// A shared set of claimed surfaces.
#[derive(Clone, Debug, Default)]
pub(crate) struct Claims {
    surfaces: Arc<Mutex<HashSet<ObjectId>>>,
}

impl Claims {
    /// Claims a surface. Returns false when the surface already has a claim.
    pub(crate) fn claim(&self, surface: ObjectId) -> bool {
        self.surfaces
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(surface)
    }

    /// Releases the claim on a surface.
    pub(crate) fn release(&self, surface: &ObjectId) {
        self.surfaces
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(surface);
    }
}
