//! `zwp_linux_dmabuf_v1` version 5 with feedback. A new dmabuf is valid when the renderer imports
//! it.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::delegate_dmabuf;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};

use crate::server::State;

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.protocols.dmabuf
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self.render.import_dmabuf(&dmabuf) {
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }

    /// Runs inside the surface state lock, so it reads only compositor-side maps.
    fn new_surface_feedback(
        &mut self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        _global: &DmabufGlobal,
    ) -> Option<smithay::wayland::dmabuf::DmabufFeedback> {
        let fullscreen = self
            .surfaces
            .id_of(surface)
            .and_then(|s| self.windows.by_surface(s))
            .and_then(|w| self.windows.get(w))
            .is_some_and(|w| w.placement == crate::shell::Placement::Fullscreen);
        if fullscreen {
            self.protocols.scanout_feedback.clone()
        } else {
            None
        }
    }
}

delegate_dmabuf!(State);
