//! Role names: normalization for queries, and the generic container roles that the compact text
//! form folds away.

/// Returns a role name in lower case with single spaces for `_`, `-` and runs of blanks.
pub fn normalize(role: &str) -> String {
    role.split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Roles that toolkits name differently for the same kind of control. A query for one name of
/// a group matches every name of it.
const SYNONYMS: &[&[&str]] = &[
    &["entry", "text box", "text", "password text", "editbar"],
    &["check box", "checkbox"],
    &["combo box", "combobox"],
    &["radio button", "radiobutton"],
    &["dialog", "alert"],
    &["link", "hyperlink"],
];

/// Returns true when a role matches a query role. The query matches the whole role or its last
/// words, so `button` matches `push button` and `toggle button`. Names of one synonym group match
/// each other, so `entry` matches the GTK 4 `text box`.
pub fn matches(role: &str, query: &str) -> bool {
    let role = normalize(role);
    let query = normalize(query);
    if query.is_empty() {
        return false;
    }
    if role == query || role.ends_with(&format!(" {query}")) {
        return true;
    }
    SYNONYMS
        .iter()
        .any(|g| g.contains(&role.as_str()) && g.contains(&query.as_str()))
}

/// Returns the AT-SPI role name of a role number, or `role <n>` for an unknown number.
pub fn name_of(role: u32) -> String {
    atspi::Role::try_from(role)
        .map(|r| r.name().to_string())
        .unwrap_or_else(|_| format!("role {role}"))
}

/// Roles that only group other nodes.
const GENERIC: &[&str] = &[
    "filler",
    "panel",
    "section",
    "grouping",
    "group",
    "unknown",
    "redundant object",
    "invalid",
    "layered pane",
    "html container",
    "generic",
    "static",
    "block quote",
];

/// Returns true for a role that only groups other nodes.
pub fn is_generic(role: &str) -> bool {
    GENERIC.contains(&normalize(role).as_str())
}

/// Roles of a toplevel window.
const FRAMES: &[&str] = &["frame", "window", "dialog", "alert", "file chooser"];

/// Returns true for the role of a toplevel window.
pub fn is_frame(role: &str) -> bool {
    FRAMES.contains(&normalize(role).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        assert_eq!(normalize("PUSH_BUTTON"), "push button");
        assert_eq!(normalize("  push-button "), "push button");
        assert_eq!(normalize("toggle  button"), "toggle button");
    }

    #[test]
    fn query_matching() {
        assert!(matches("push button", "button"));
        assert!(matches("push button", "push_button"));
        assert!(matches("toggle button", "Button"));
        assert!(!matches("push button", "push"));
        assert!(!matches("button box", "button"));
        assert!(!matches("label", ""));
        assert!(matches("entry", "entry"));
        assert!(matches("text box", "entry"));
        assert!(matches("entry", "text"));
        assert!(matches("check box", "checkbox"));
        assert!(!matches("text box", "label"));
    }

    #[test]
    fn role_numbers_map_to_names() {
        assert_eq!(name_of(43), "button");
        assert_eq!(name_of(23), "frame");
        assert_eq!(name_of(100_000), "role 100000");
    }

    #[test]
    fn generic_and_frame_roles() {
        assert!(is_generic("filler"));
        assert!(is_generic("Panel"));
        assert!(!is_generic("push button"));
        assert!(is_frame("frame"));
        assert!(is_frame("dialog"));
        assert!(!is_frame("label"));
    }
}
