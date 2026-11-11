//! `wp_cursor_shape_manager_v1`: named cursor shapes. A named shape arrives through
//! `SeatHandler::cursor_image`. Tablet tool shapes go through the tablet seat handler in
//! `tablet.rs`.

use smithay::delegate_cursor_shape;

use crate::server::State;

delegate_cursor_shape!(State);
