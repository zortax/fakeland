//! `fakeland ctl a11y`: the accessibility tree of the session. Node ids are the short names
//! that the server prints, such as `n12`; they stay valid while the server runs.

use anyhow::Result;
use clap::{Args, Subcommand};
use fakeland::accessibility::{Node, Query, render_with};
use fakeland::{A11yTarget, ClickVia, TextMethod};
use fakeland_control::ControlClient;
use fakeland_control::methods::*;
use fakeland_control::spec::parse_window_ref;
use fakeland_scenario::WindowRef;
use fakeland_types::ProcessId;

use super::args::CtlCommand;
use super::print::{emit, sent_text};

/// What an a11y command reads.
#[derive(Debug, Args)]
pub struct A11yScope {
    /// A window: focused (the default), an id, app:ID or title:REGEX.
    #[arg(long, value_name = "W")]
    pub window: Option<String>,
    /// An app: a pid or a part of the accessible app name.
    #[arg(long, value_name = "APP", conflicts_with = "window")]
    pub app: Option<String>,
    /// The subtree of a node.
    #[arg(long, value_name = "ID")]
    pub node: Option<String>,
    /// Keep nodes that are not showing.
    #[arg(long)]
    pub all: bool,
}

impl A11yScope {
    fn target(&self) -> Result<A11yTarget> {
        if let Some(app) = &self.app {
            return Ok(match app.trim().parse::<u32>() {
                Ok(pid) => A11yTarget::Pid {
                    pid: ProcessId(pid),
                },
                Err(_) => A11yTarget::App { name: app.clone() },
            });
        }
        let window = match &self.window {
            Some(w) => parse_window_ref(w)?,
            None => WindowRef::Focused,
        };
        Ok(A11yTarget::Window { window })
    }
}

/// A `fakeland ctl a11y` command.
#[derive(Debug, Subcommand)]
pub enum A11yCommand {
    /// List the accessible apps and their frames.
    Apps,
    /// Print the tree of a window, an app or a node.
    Tree {
        /// What to read.
        #[command(flatten)]
        scope: A11yScope,
        /// The deepest level below the root.
        #[arg(long, default_value_t = 30)]
        depth: usize,
        /// The most nodes.
        #[arg(long, default_value_t = 1500)]
        nodes: usize,
    },
    /// Find nodes by role, name, text, state and action.
    Find {
        /// Where to search.
        #[command(flatten)]
        scope: A11yScope,
        /// The role, for example button or entry.
        #[arg(long)]
        role: Option<String>,
        /// A case-insensitive regular expression for the name.
        #[arg(long)]
        name: Option<String>,
        /// A case-insensitive regular expression for name, description or text.
        #[arg(long)]
        text: Option<String>,
        /// A required state; `!state` requires its absence. Repeatable.
        #[arg(long = "state", value_name = "STATE")]
        states: Vec<String>,
        /// An action the node offers.
        #[arg(long)]
        action: Option<String>,
        /// The most results.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Print one node.
    Node {
        /// The node id.
        id: String,
    },
    /// Click a node through its action, or with the pointer.
    Click {
        /// The node id.
        id: String,
        /// Click the center of the node bounds with the pointer.
        #[arg(long, conflicts_with = "action_only")]
        pointer: bool,
        /// Fail when the node has no click action.
        #[arg(long)]
        action_only: bool,
    },
    /// Run a named action of a node.
    Act {
        /// The node id.
        id: String,
        /// The action name.
        action: String,
    },
    /// Replace the text of a node.
    SetText {
        /// The node id.
        id: String,
        /// The new text.
        text: String,
    },
    /// Move keyboard focus to a node.
    Focus {
        /// The node id.
        id: String,
    },
}

fn found_text(f: &A11yFound) -> String {
    if f.nodes.is_empty() {
        "no node matches".into()
    } else {
        f.text.clone()
    }
}

fn acted_text(id: &str, a: &A11yActed) -> String {
    match (a.via, a.point) {
        (Some(ClickVia::Pointer), Some(p)) => format!(
            "clicked {id} with the pointer at {:.0},{:.0}: {}",
            p.x,
            p.y,
            sent_text(&a.sent)
        ),
        _ => format!("ran action {:?} of {id}", a.action.as_deref().unwrap_or("")),
    }
}

/// Renders one node under the id the caller gave.
fn node_text(id: &str, n: &Node) -> String {
    render_with(n, &Default::default(), &mut |_| id.to_string())
        .trim_end()
        .to_string()
}

fn apps_text(apps: &[fakeland::accessibility::AppInfo]) -> String {
    let mut out = String::new();
    for a in apps {
        let pid = a.pid.map_or_else(|| "?".into(), |p| p.0.to_string());
        out.push_str(&format!(
            "{:?} pid {pid} {} #{}\n",
            a.name, a.toolkit, a.root
        ));
        for f in &a.frames {
            let window = f.window.map_or_else(String::new, |w| format!(" {w}"));
            out.push_str(&format!("  {} {:?} #{}{window}\n", f.role, f.name, f.node));
        }
    }
    if out.is_empty() {
        out.push_str("no accessible apps\n");
    }
    out
}

/// Runs an a11y command, or returns `None` for other commands.
pub fn run(client: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    let CtlCommand::A11y(cmd) = cmd else {
        return Ok(None);
    };
    match cmd {
        A11yCommand::Apps => {
            let apps = client.blocking_call::<A11yApps>(NoParams {})?;
            emit(json, &apps, |a| apps_text(a))?;
        }
        A11yCommand::Tree {
            scope,
            depth,
            nodes,
        } => {
            let out = client.blocking_call::<A11yTreeOf>(A11yTreeParams {
                target: scope.target()?,
                root: scope.node.clone(),
                limits: A11yLimits {
                    max_depth: *depth,
                    max_nodes: (*nodes).max(1),
                    all: scope.all,
                    ..A11yLimits::default()
                },
            })?;
            emit(json, &out, |t| t.text.clone())?;
        }
        A11yCommand::Find {
            scope,
            role,
            name,
            text,
            states,
            action,
            limit,
        } => {
            let out = client.blocking_call::<A11yFind>(A11yFindParams {
                target: scope.target()?,
                root: scope.node.clone(),
                query: Query {
                    role: role.clone(),
                    name: name.clone(),
                    text: text.clone(),
                    states: states.clone(),
                    action: action.clone(),
                    interface: None,
                },
                limit: (*limit).max(1),
                limits: A11yLimits {
                    all: scope.all,
                    ..A11yLimits::default()
                },
            })?;
            emit(json, &out, found_text)?;
            if out.nodes.is_empty() {
                return Ok(Some(1));
            }
        }
        A11yCommand::Node { id } => {
            let n = client.blocking_call::<A11yNode>(A11yNodeParams { node: id.clone() })?;
            emit(json, &n, |n| node_text(id, n))?;
        }
        A11yCommand::Click {
            id,
            pointer,
            action_only,
        } => {
            let via = match (pointer, action_only) {
                (true, _) => ClickVia::Pointer,
                (_, true) => ClickVia::Action,
                _ => ClickVia::Auto,
            };
            let out = client.blocking_call::<A11yAction>(A11yActionParams {
                node: id.clone(),
                action: "click".into(),
                via,
            })?;
            emit(json, &out, |a| acted_text(id, a))?;
        }
        A11yCommand::Act { id, action } => {
            let out = client.blocking_call::<A11yAction>(A11yActionParams {
                node: id.clone(),
                action: action.clone(),
                via: ClickVia::Action,
            })?;
            emit(json, &out, |a| acted_text(id, a))?;
        }
        A11yCommand::SetText { id, text } => {
            let out = client.blocking_call::<A11ySetText>(A11ySetTextParams {
                node: id.clone(),
                text: text.clone(),
            })?;
            emit(json, &out, |o| match o.method {
                TextMethod::EditableText => format!("set the text of {id} through EditableText"),
                TextMethod::Typed => format!("typed the text into {id}"),
            })?;
        }
        A11yCommand::Focus { id } => {
            client.blocking_call::<A11yFocus>(A11yNodeParams { node: id.clone() })?;
            emit(json, &(), |_| format!("focused {id}"))?;
        }
    }
    Ok(Some(0))
}
