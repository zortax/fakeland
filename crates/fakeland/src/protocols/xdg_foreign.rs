//! `zxdg_exporter_v2` and `zxdg_importer_v2`: cross-client parent relations of toplevels.

use smithay::delegate_xdg_foreign;
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};

use crate::server::State;

impl XdgForeignHandler for State {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.protocols.xdg_foreign
    }
}

delegate_xdg_foreign!(State);
