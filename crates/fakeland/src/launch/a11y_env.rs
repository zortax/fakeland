//! Toolkit switches for accessibility in a child environment. With accessibility on, GTK uses its
//! AT-SPI backend and Qt exposes its tree without waiting for a screen reader; variables that turn
//! the bridges off are removed. With accessibility off, the bridges stay off, so no toolkit starts
//! an accessibility bus through bus activation. Chromium and Electron build their tree when an
//! assistive client asks for it; `--force-renderer-accessibility` on their command line builds it
//! from the start.

use std::collections::BTreeMap;

/// Variables set with accessibility on.
const ENABLE: &[(&str, &str)] = &[
    ("GTK_A11Y", "atspi"),
    ("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1"),
];

/// Variables removed with accessibility on, because they turn the bridges off.
const DISABLERS: &[&str] = &["NO_AT_BRIDGE", "QT_ACCESSIBILITY", "AT_SPI_BUS_ADDRESS"];

/// Variables set with accessibility off.
const DISABLE: &[(&str, &str)] = &[("NO_AT_BRIDGE", "1"), ("GTK_A11Y", "none")];

/// Applies the accessibility switches to an environment.
pub fn apply(env: &mut BTreeMap<String, String>, accessibility: bool) {
    if accessibility {
        for k in DISABLERS {
            env.remove(*k);
        }
        for (k, v) in ENABLE {
            env.insert((*k).into(), (*v).into());
        }
    } else {
        env.remove("QT_LINUX_ACCESSIBILITY_ALWAYS_ON");
        for (k, v) in DISABLE {
            env.insert((*k).into(), (*v).into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_sets_enablers_and_drops_disablers() {
        let mut env: BTreeMap<String, String> = [("NO_AT_BRIDGE", "1"), ("GTK_A11Y", "none")]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        apply(&mut env, true);
        assert_eq!(env["GTK_A11Y"], "atspi");
        assert_eq!(env["QT_LINUX_ACCESSIBILITY_ALWAYS_ON"], "1");
        assert!(!env.contains_key("NO_AT_BRIDGE"));
    }

    #[test]
    fn off_turns_the_bridges_off() {
        let mut env = BTreeMap::new();
        env.insert(
            "QT_LINUX_ACCESSIBILITY_ALWAYS_ON".to_string(),
            "1".to_string(),
        );
        apply(&mut env, false);
        assert_eq!(env["NO_AT_BRIDGE"], "1");
        assert_eq!(env["GTK_A11Y"], "none");
        assert!(!env.contains_key("QT_LINUX_ACCESSIBILITY_ALWAYS_ON"));
    }
}
