//! The identity of the fakeland_marks_v1 global: its interface name and the version that these
//! bindings implement.

/// The interface name of the global, as the registry advertises it.
pub const MANAGER_INTERFACE: &str = "fakeland_marks_manager_v1";

/// The highest version of the global that these bindings implement.
pub const MANAGER_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "client")]
    #[test]
    fn client_bindings_match_the_constants() {
        use crate::client::fakeland_marks_manager_v1::FakelandMarksManagerV1;
        use wayland_client::Proxy;
        let iface = FakelandMarksManagerV1::interface();
        assert_eq!(iface.name, MANAGER_INTERFACE);
        assert_eq!(iface.version, MANAGER_VERSION);
    }

    #[cfg(feature = "server")]
    #[test]
    fn server_bindings_match_the_constants() {
        use crate::server::fakeland_marks_manager_v1::FakelandMarksManagerV1;
        use wayland_server::Resource;
        let iface = FakelandMarksManagerV1::interface();
        assert_eq!(iface.name, MANAGER_INTERFACE);
        assert_eq!(iface.version, MANAGER_VERSION);
    }
}
