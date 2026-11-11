//! `wp_single_pixel_buffer_manager_v1`: one-pixel color buffers.

use smithay::delegate_single_pixel_buffer;

use crate::server::State;

delegate_single_pixel_buffer!(State);
