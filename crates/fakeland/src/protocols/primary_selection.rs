//! `zwp_primary_selection_device_manager_v1`. Selections share the handler of the data device.

use smithay::delegate_primary_selection;
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};

use crate::server::State;

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.protocols.primary_selection
    }
}

delegate_primary_selection!(State);
