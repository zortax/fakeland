//! The environment of a child process: the inherited variables minus the host display and the
//! variables of the host fakeland process, plus the session socket, toolkit backend hints, the
//! session bus and extra variables.

use std::collections::BTreeMap;

/// Variables that point a child at the host desktop. Children never inherit them.
const HOST_VARIABLES: &[&str] = &[
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "DISPLAY",
    "XAUTHORITY",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "HYPRLAND_CMD",
    "SWAYSOCK",
    "I3SOCK",
    "NIRI_SOCKET",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP",
    "DESKTOP_SESSION",
    "GNOME_SETUP_DISPLAY",
    "DESKTOP_STARTUP_ID",
    "XDG_ACTIVATION_TOKEN",
];

/// The prefix of the variables that configure fakeland itself. Children never inherit them.
const OWN_PREFIX: &str = "FAKELAND_";

/// Toolkit hints that select the Wayland backend.
const BACKEND_HINTS: &[(&str, &str)] = &[
    ("XDG_SESSION_TYPE", "wayland"),
    ("GDK_BACKEND", "wayland"),
    ("QT_QPA_PLATFORM", "wayland"),
    ("SDL_VIDEODRIVER", "wayland"),
    ("ELECTRON_OZONE_PLATFORM_HINT", "wayland"),
    ("MOZ_ENABLE_WAYLAND", "1"),
];

/// The inputs that shape a child environment.
#[derive(Debug, Clone)]
pub struct ChildEnv<'a> {
    /// The socket name the child connects to.
    pub socket: &'a str,
    /// The private bus address. `None` keeps the inherited bus.
    pub dbus_address: Option<&'a str>,
    /// Session-wide extra variables.
    pub session_env: &'a BTreeMap<String, String>,
    /// Per-launch variables. They override everything else. Session and launch variables reach
    /// the child even when their names start with `FAKELAND_`.
    pub launch_env: &'a [(String, String)],
    /// Toolkits expose their accessibility trees.
    pub accessibility: bool,
}

impl ChildEnv<'_> {
    /// Builds the full environment from an inherited one.
    pub fn build(
        &self,
        inherited: impl IntoIterator<Item = (String, String)>,
    ) -> BTreeMap<String, String> {
        let mut env: BTreeMap<String, String> = inherited
            .into_iter()
            .filter(|(k, _)| !HOST_VARIABLES.contains(&k.as_str()) && !k.starts_with(OWN_PREFIX))
            .collect();
        for (k, v) in BACKEND_HINTS {
            env.insert((*k).into(), (*v).into());
        }
        env.insert("WAYLAND_DISPLAY".into(), self.socket.into());
        if let Some(addr) = self.dbus_address {
            env.insert("DBUS_SESSION_BUS_ADDRESS".into(), addr.into());
            env.remove("AT_SPI_BUS_ADDRESS");
        }
        super::a11y_env::apply(&mut env, self.accessibility);
        for (k, v) in self.session_env {
            env.insert(k.clone(), v.clone());
        }
        for (k, v) in self.launch_env {
            env.insert(k.clone(), v.clone());
        }
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Vec<(String, String)> {
        [
            ("WAYLAND_DISPLAY", "wayland-1"),
            ("DISPLAY", ":0"),
            ("HYPRLAND_INSTANCE_SIGNATURE", "x"),
            ("HOME", "/home/u"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
            ("GDK_BACKEND", "x11"),
            ("FAKELAND_SOCKET", "/run/user/1000/fakeland-1.sock"),
            ("FAKELAND_LOG", "debug"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
    }

    #[test]
    fn removes_host_display_and_sets_hints() {
        let session_env = BTreeMap::new();
        let env = ChildEnv {
            socket: "wayland-7",
            dbus_address: Some("unix:path=/tmp/dbus-x"),
            session_env: &session_env,
            launch_env: &[],
            accessibility: true,
        }
        .build(host());
        assert_eq!(env["WAYLAND_DISPLAY"], "wayland-7");
        assert!(!env.contains_key("DISPLAY"));
        assert!(!env.contains_key("HYPRLAND_INSTANCE_SIGNATURE"));
        assert_eq!(env["GDK_BACKEND"], "wayland");
        assert_eq!(env["MOZ_ENABLE_WAYLAND"], "1");
        assert_eq!(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/tmp/dbus-x");
        assert_eq!(env["HOME"], "/home/u");
        assert!(env.keys().all(|k| !k.to_lowercase().contains("fakeland")));
    }

    #[test]
    fn host_bus_and_overrides() {
        let mut session_env = BTreeMap::new();
        session_env.insert("A".to_string(), "session".to_string());
        session_env.insert("B".to_string(), "session".to_string());
        let launch = [
            ("B".to_string(), "launch".to_string()),
            ("FAKELAND_TESTCLIENT".to_string(), "set".to_string()),
        ];
        let env = ChildEnv {
            socket: "wayland-3",
            dbus_address: None,
            session_env: &session_env,
            launch_env: &launch,
            accessibility: false,
        }
        .build(host());
        assert_eq!(
            env["DBUS_SESSION_BUS_ADDRESS"],
            "unix:path=/run/user/1000/bus"
        );
        assert_eq!(env["A"], "session");
        assert_eq!(env["B"], "launch");
        assert_eq!(env["FAKELAND_TESTCLIENT"], "set");
        assert!(!env.contains_key("FAKELAND_SOCKET"));
    }
}
