//! The compact text form of a tree for agents: one line per node with role, name, id, bounds,
//! actions and the states that matter for interaction. Unnamed generic containers fold away and
//! their children move up one level.

use std::fmt::Write;

use crate::{Node, NodeRef, roles};

/// States that the text form shows.
const SHOWN: &[&str] = &[
    "focused",
    "active",
    "checked",
    "selected",
    "pressed",
    "expanded",
    "collapsed",
    "indeterminate",
    "modal",
    "required",
    "invalid-entry",
    "read-only",
    "busy",
];

/// Switches of the text form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextOptions {
    /// Unnamed generic containers fold away.
    pub fold_generic: bool,
    /// The most characters of text contents per line.
    pub text_chars: usize,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            fold_generic: true,
            text_chars: 80,
        }
    }
}

/// Names a node in the text form.
pub type IdFn<'a> = dyn FnMut(&NodeRef) -> String + 'a;

/// Renders a tree with full node ids.
pub fn render(node: &Node, opts: &TextOptions) -> String {
    render_with(node, opts, &mut |id| id.to_string())
}

/// Renders a tree and names each node through `ids`.
pub fn render_with(node: &Node, opts: &TextOptions, ids: &mut IdFn<'_>) -> String {
    let mut out = String::new();
    render_at(node, 0, opts, ids, &mut out);
    out
}

/// Renders nodes one per line without their children, with full node ids.
pub fn render_list(nodes: &[Node], opts: &TextOptions) -> String {
    render_list_with(nodes, opts, &mut |id| id.to_string())
}

/// Renders nodes one per line without their children and names each node through `ids`.
pub fn render_list_with(nodes: &[Node], opts: &TextOptions, ids: &mut IdFn<'_>) -> String {
    let mut out = String::new();
    for n in nodes {
        line(n, 0, opts, ids, &mut out);
    }
    out
}

/// Renders the line of one node with its full id.
pub fn render_line(node: &Node, opts: &TextOptions) -> String {
    let mut out = String::new();
    line(node, 0, opts, &mut |id| id.to_string(), &mut out);
    out.trim_end().to_string()
}

fn render_at(node: &Node, depth: usize, opts: &TextOptions, ids: &mut IdFn<'_>, out: &mut String) {
    let folded = opts.fold_generic && depth > 0 && foldable(node);
    let child_depth = if folded { depth } else { depth + 1 };
    if !folded {
        line(node, depth, opts, ids, out);
    }
    for c in &node.children {
        render_at(c, child_depth, opts, ids, out);
    }
    if node.omitted > 0 {
        let _ = writeln!(out, "{}… {} more", "  ".repeat(child_depth), node.omitted);
    }
}

fn foldable(node: &Node) -> bool {
    roles::is_generic(&node.role)
        && node.name.is_empty()
        && shown_actions(node).is_empty()
        && node.text.as_deref().is_none_or(str::is_empty)
        && node.error.is_none()
}

/// Returns the action names that the text form shows: names without a dot. Dotted names are
/// toolkit-internal widget actions such as `clipboard.copy`.
pub fn shown_actions(node: &Node) -> Vec<&str> {
    node.actions
        .iter()
        .map(String::as_str)
        .filter(|a| !a.is_empty() && !a.contains('.'))
        .collect()
}

fn line(node: &Node, depth: usize, opts: &TextOptions, ids: &mut IdFn<'_>, out: &mut String) {
    let role = if node.role.is_empty() {
        "node"
    } else {
        &node.role
    };
    let _ = write!(out, "{}- {role}", "  ".repeat(depth));
    if !node.name.is_empty() {
        let _ = write!(out, " {}", quote(&node.name, opts.text_chars));
    }
    if let Some(t) = node.text.as_deref()
        && !t.trim().is_empty()
        && t != node.name
    {
        let _ = write!(out, " = {}", quote(t, opts.text_chars));
    }
    if let Some(v) = node.value {
        let _ = write!(out, " = {}", v.current);
        if let (Some(min), Some(max)) = (v.min, v.max) {
            let _ = write!(out, " [{min}..{max}]");
        }
    }
    let _ = write!(out, " #{}", ids(&node.id));
    if let Some(b) = node.bounds {
        let _ = write!(out, " @{},{} {}x{}", b.x, b.y, b.w, b.h);
    }
    let actions = shown_actions(node);
    if !actions.is_empty() {
        let _ = write!(out, " [{}]", actions.join(", "));
    }
    let states = shown_states(node);
    if !states.is_empty() {
        let _ = write!(out, " ({})", states.join(", "));
    }
    if let Some(e) = &node.error {
        let _ = write!(out, " error: {e}");
    }
    out.push('\n');
}

/// Returns the states that the text form shows, plus `disabled` and `hidden`.
pub fn shown_states(node: &Node) -> Vec<&str> {
    let mut list: Vec<&str> = SHOWN
        .iter()
        .copied()
        .filter(|s| node.has_state(s))
        .collect();
    if node.has_state("editable") && !node.has_state("read-only") {
        list.push("editable");
    }
    let interactive = !shown_actions(node).is_empty() || node.has_state("focusable");
    if interactive && !node.has_state("enabled") && !node.has_state("sensitive") {
        list.push("disabled");
    }
    if !node.states.is_empty() && !node.has_state("showing") {
        list.push("hidden");
    }
    list
}

fn quote(s: &str, limit: usize) -> String {
    let mut t: String = s.chars().take(limit).collect();
    if s.chars().count() > limit {
        t.push('…');
    }
    format!("{t:?}")
}

#[cfg(test)]
mod tests {
    use fakeland_types::Rect;

    use super::*;
    use crate::{NodeRef, NodeValue};

    fn node(id: &str, role: &str, name: &str) -> Node {
        let mut n = Node::empty(NodeRef::new(
            ":1.4",
            format!("/org/a11y/atspi/accessible/{id}"),
        ));
        n.role = role.into();
        n.name = name.into();
        n.states = vec!["enabled".into(), "showing".into()];
        n
    }

    #[test]
    fn tree_lines_fold_generic_containers() {
        let mut frame = node("1", "frame", "Dialog");
        frame.states.push("active".into());
        frame.bounds = Some(Rect::new(10, 20, 300, 200));
        let mut panel = node("2", "panel", "");
        let mut ok = node("3", "push button", "OK");
        ok.actions = vec!["click".into()];
        ok.states.push("focused".into());
        ok.bounds = Some(Rect::new(200, 180, 60, 30));
        panel.children.push(ok);
        let mut label = node("4", "label", "Done");
        label.text = Some("Done".into());
        panel.children.push(label);
        panel.omitted = 3;
        frame.children.push(panel);
        let text = render(&frame, &TextOptions::default());
        assert_eq!(
            text,
            "- frame \"Dialog\" #1.4:1 @10,20 300x200 (active)\n\
             \x20 - push button \"OK\" #1.4:3 @200,180 60x30 [click] (focused)\n\
             \x20 - label \"Done\" #1.4:4\n\
             \x20 … 3 more\n"
        );
        let flat = render(
            &frame,
            &TextOptions {
                fold_generic: false,
                ..TextOptions::default()
            },
        );
        assert!(flat.contains("  - panel #1.4:2\n    - push button"));
    }

    #[test]
    fn text_value_and_derived_states() {
        let mut entry = node("5", "entry", "Name");
        entry.text = Some("a\nlong text that goes on".into());
        entry.states = vec!["editable".into(), "focusable".into()];
        let line = render_line(
            &entry,
            &TextOptions {
                text_chars: 6,
                ..TextOptions::default()
            },
        );
        assert_eq!(
            line,
            "- entry \"Name\" = \"a\\nlong…\" #1.4:5 (editable, disabled, hidden)"
        );
        let mut slider = node("6", "slider", "");
        slider.value = Some(NodeValue {
            current: 5.0,
            min: Some(0.0),
            max: Some(10.0),
        });
        assert_eq!(
            render_list(&[slider], &TextOptions::default()),
            "- slider = 5 [0..10] #1.4:6\n"
        );
    }

    #[test]
    fn dotted_actions_stay_out_and_ids_come_from_the_namer() {
        let mut label = node("8", "label", "Hello");
        label.actions = vec!["clipboard.copy".into(), "menu.popup".into()];
        label.states.push("focusable".into());
        let mut names = Vec::new();
        let text = render_with(&label, &TextOptions::default(), &mut |id| {
            names.push(id.clone());
            "n1".into()
        });
        assert_eq!(text, "- label \"Hello\" #n1\n");
        assert_eq!(names.len(), 1);
        assert_eq!(shown_actions(&label), Vec::<&str>::new());
    }

    #[test]
    fn errors_show_on_the_line() {
        let mut n = node("7", "", "");
        n.states.clear();
        n.error = Some("timeout".into());
        assert_eq!(
            render_line(&n, &TextOptions::default()),
            "- node #1.4:7 error: timeout"
        );
    }
}
