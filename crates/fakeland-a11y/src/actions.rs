//! Action name resolution. A request names an action of the node. The activation names `click`,
//! `press`, `activate`, `doDefault`, `jump`, `toggle`, `open` and `select` stand in for each
//! other, because toolkits name the default action of a control differently.

/// Activation names in order of preference, in lower case.
const ACTIVATE: &[&str] = &[
    "click",
    "press",
    "activate",
    "dodefault",
    "jump",
    "toggle",
    "open",
    "select",
];

/// Returns the index of the action that a request names.
pub fn resolve(names: &[String], wanted: &str) -> Option<usize> {
    let wanted = wanted.trim().to_lowercase();
    if let Some(i) = names.iter().position(|n| n.to_lowercase() == wanted) {
        return Some(i);
    }
    if !ACTIVATE.contains(&wanted.as_str()) {
        return None;
    }
    ACTIVATE
        .iter()
        .find_map(|a| names.iter().position(|n| n.to_lowercase() == *a))
}

/// Returns true when a node with these actions can be activated.
pub fn can_activate(names: &[String]) -> bool {
    resolve(names, "click").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_names_win() {
        let n = names(&["press", "click"]);
        assert_eq!(resolve(&n, "click"), Some(1));
        assert_eq!(resolve(&n, "PRESS"), Some(0));
    }

    #[test]
    fn activation_names_stand_in_for_each_other() {
        assert_eq!(resolve(&names(&["activate"]), "click"), Some(0));
        assert_eq!(
            resolve(&names(&["expand or contract", "jump"]), "press"),
            Some(1)
        );
        assert_eq!(
            resolve(&names(&["showContextMenu", "doDefault"]), "click"),
            Some(1)
        );
        assert!(can_activate(&names(&["toggle"])));
        assert!(!can_activate(&names(&["expand or contract"])));
    }

    #[test]
    fn other_names_need_an_exact_match() {
        let n = names(&["expand or contract", "click"]);
        assert_eq!(resolve(&n, "expand or contract"), Some(0));
        assert_eq!(resolve(&n, "expand"), None);
        assert_eq!(resolve(&[], "click"), None);
    }
}
