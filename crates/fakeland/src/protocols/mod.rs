//! Wayland protocol handlers, one file per protocol, and the struct that owns every protocol
//! state.
//!
//! To add a global: write a handler file here with its delegate macro, add its state to
//! `Protocols` in `state.rs` and create it in `Protocols::new` in `globals.rs`.

mod alpha_modifier;
mod commit_timing;
mod compositor;
mod content_type;
mod cursor_shape;
mod data_device;
mod decoration;
mod dmabuf;
mod fifo;
mod fractional_scale;
mod globals;
mod idle_inhibit;
mod input_method;
mod keyboard_shortcuts_inhibit;
mod marks_manager;
mod output;
mod pointer_constraints;
mod pointer_gestures;
mod presentation;
mod primary_selection;
mod relative_pointer;
mod seat;
mod shm;
mod single_pixel;
mod state;
mod surface_marks;
mod syncobj;
mod system_bell;
mod tablet;
mod tearing_control;
mod text_input;
mod toplevel_tag;
mod viewporter;
mod xdg_activation;
mod xdg_foreign;
mod xdg_shell;

pub use state::Protocols;
pub use tearing_control::TearingControlState;
pub use text_input::create_text_input_global;
