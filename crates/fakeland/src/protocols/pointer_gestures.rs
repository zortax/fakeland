//! `zwp_pointer_gestures_v1`: swipe, pinch and hold gestures.

use smithay::delegate_pointer_gestures;

use crate::server::State;

delegate_pointer_gestures!(State);
