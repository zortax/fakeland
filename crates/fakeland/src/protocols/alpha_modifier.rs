//! `wp_alpha_modifier_v1`: per-surface opacity multipliers.

use smithay::delegate_alpha_modifier;

use crate::server::State;

delegate_alpha_modifier!(State);
