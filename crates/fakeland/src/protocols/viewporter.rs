//! `wp_viewporter`: crop and scale of surfaces. Smithay applies the viewport state.

use smithay::delegate_viewporter;

use crate::server::State;

delegate_viewporter!(State);
