//! wp_viewporter and wp_viewport, which have no events.

use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};

use crate::app::App;

wayland_client::delegate_noop!(App: WpViewporter);
wayland_client::delegate_noop!(App: WpViewport);
