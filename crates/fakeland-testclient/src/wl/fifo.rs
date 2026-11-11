//! wp_fifo_manager_v1 and wp_fifo_v1, which have no events.

use wayland_protocols::wp::fifo::v1::client::{
    wp_fifo_manager_v1::WpFifoManagerV1, wp_fifo_v1::WpFifoV1,
};

use crate::app::App;

wayland_client::delegate_noop!(App: WpFifoManagerV1);
wayland_client::delegate_noop!(App: WpFifoV1);
