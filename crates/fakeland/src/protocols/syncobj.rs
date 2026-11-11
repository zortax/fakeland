//! `wp_linux_drm_syncobj_manager_v1`: explicit sync on the render node. The commit hook watches
//! acquire points, and released buffers signal their release points.

use smithay::delegate_drm_syncobj;
use smithay::wayland::drm_syncobj::{DrmSyncobjHandler, DrmSyncobjState};

use crate::server::State;

impl DrmSyncobjHandler for State {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.protocols.syncobj.as_mut()
    }
}

delegate_drm_syncobj!(State);
