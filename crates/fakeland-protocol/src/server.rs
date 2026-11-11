//! Server-side bindings of fakeland_marks_v1, generated from the protocol XML.

#![allow(
    missing_docs,
    non_upper_case_globals,
    non_camel_case_types,
    unused_imports
)]
#![allow(clippy::all)]

use wayland_server;
use wayland_server::protocol::*;

pub mod __interfaces {
    //! Interface descriptions of fakeland_marks_v1 for the server backend.
    use wayland_server::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("protocol/fakeland-marks-v1.xml");
}
use self::__interfaces::*;

wayland_scanner::generate_server_code!("protocol/fakeland-marks-v1.xml");
