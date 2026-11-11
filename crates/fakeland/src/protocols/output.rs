//! `wl_output` and `zxdg_output_manager_v1`. Smithay sends the mode, geometry, scale, name and
//! description of each faked output.

use smithay::delegate_output;
use smithay::wayland::output::OutputHandler;

use crate::server::State;

impl OutputHandler for State {}

delegate_output!(State);
