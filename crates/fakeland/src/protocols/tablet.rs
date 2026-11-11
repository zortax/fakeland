//! `zwp_tablet_manager_v2`. The seat has no tablet, as on a desktop without a tablet attached, so
//! tool cursor images only exist.

use smithay::delegate_tablet_manager;
use smithay::wayland::tablet_manager::TabletSeatHandler;

use crate::server::State;

impl TabletSeatHandler for State {}

delegate_tablet_manager!(State);
