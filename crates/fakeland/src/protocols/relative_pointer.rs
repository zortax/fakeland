//! `zwp_relative_pointer_manager_v1`: unaccelerated pointer motion.

use smithay::delegate_relative_pointer;

use crate::server::State;

delegate_relative_pointer!(State);
