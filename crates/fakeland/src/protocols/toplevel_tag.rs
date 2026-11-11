//! `xdg_toplevel_tag_manager_v1`. Toplevel tags and their descriptions only exist; window
//! matching uses the app id and the title.

use smithay::delegate_xdg_toplevel_tag;
use smithay::wayland::xdg_toplevel_tag::XdgToplevelTagHandler;

use crate::server::State;

impl XdgToplevelTagHandler for State {}

delegate_xdg_toplevel_tag!(State);
