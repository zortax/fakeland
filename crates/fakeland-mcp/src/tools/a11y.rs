//! Accessibility tools: the tree of a window or app, node search, actions and text changes.

use anyhow::{Context, Result};
use fakeland::accessibility::Query;
use fakeland::{A11yTarget, ClickVia, TextMethod};
use fakeland_control::methods::{
    A11yActionParams as CtlAction, A11yFindParams as CtlFind, A11yLimits,
    A11ySetTextParams as CtlSetText, A11yTreeParams as CtlTree,
};
use fakeland_scenario::WindowRef;
use fakeland_types::ProcessId;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_router};

use super::common::matcher;
use crate::params::{
    A11yActionParams, A11yFindParams, A11yScopeParams, A11ySetTextParams, A11yTreeParams,
};
use crate::reply::{ToolResult, done, finish};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

/// The time limit of a tree walk.
const WALK_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The legend that follows a tree.
const LEGEND: &str = "Each line: role \"name\" = text #id @x,y wxh [actions] (states). \
Coordinates are logical output coordinates. Pass #id without # as node to a11y_action or \
a11y_set_text.";

fn target(p: &A11yScopeParams) -> Result<A11yTarget> {
    if let Some(app) = &p.app {
        return Ok(match app.trim().parse::<u32>() {
            Ok(pid) => A11yTarget::Pid {
                pid: ProcessId(pid),
            },
            Err(_) => A11yTarget::App { name: app.clone() },
        });
    }
    Ok(A11yTarget::Window {
        window: matcher(p.window.as_deref())?.unwrap_or(WindowRef::Focused),
    })
}

fn via(name: Option<&str>) -> Result<ClickVia> {
    match name {
        None => Ok(ClickVia::Auto),
        Some(n) => serde_json::from_value(serde_json::json!(n.trim().to_ascii_lowercase()))
            .with_context(|| format!("unknown via {n:?}: use auto, action or pointer")),
    }
}

#[tool_router(router = a11y_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Returns the accessibility tree.
    #[tool(
        description = "Read the accessibility tree (AT-SPI) of the focused window, another window, an app or a node. One line per element: role, name, text, #id, logical bounds @x,y wxh, actions and states. Use the ids with a11y_action and a11y_set_text. By default only elements showing on screen are listed and unnamed containers fold away. GTK, Qt and Chromium/Electron apps expose trees; Chromium needs --force-renderer-accessibility on its command line for web content."
    )]
    async fn a11y_tree(&self, Parameters(p): Parameters<A11yTreeParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let params = CtlTree {
                    target: target(&p.scope)?,
                    root: p.scope.node.clone(),
                    limits: A11yLimits {
                        max_depth: p.max_depth,
                        max_nodes: p.max_nodes.max(1),
                        all: p.scope.all,
                        ..A11yLimits::default()
                    },
                };
                let out = bounded(WALK_LIMIT, c.a11y_tree(params)).await??;
                let mut content = vec![ContentBlock::text(format!(
                    "{} elements.\n{}",
                    out.nodes, out.text
                ))];
                content.push(ContentBlock::text(LEGEND));
                if p.json {
                    content.push(ContentBlock::text(serde_json::to_string_pretty(&out.tree)?));
                }
                Ok(CallToolResult::success(content))
            }
            .await,
        )
    }

    /// Searches elements.
    #[tool(
        description = "Find accessibility elements by role (\"button\" also matches \"push button\" and \"toggle button\"), name or text (case-insensitive regex), states (\"focused\", \"checked\", \"!showing\") and action. Returns one line per match with its #id and logical bounds."
    )]
    async fn a11y_find(&self, Parameters(p): Parameters<A11yFindParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let params = CtlFind {
                    target: target(&p.scope)?,
                    root: p.scope.node.clone(),
                    query: Query {
                        role: p.role.clone(),
                        name: p.name.clone(),
                        text: p.text.clone(),
                        states: p.states.clone(),
                        action: p.action.clone(),
                        interface: None,
                    },
                    limit: p.limit.max(1),
                    limits: A11yLimits {
                        all: p.scope.all,
                        ..A11yLimits::default()
                    },
                };
                let out = bounded(WALK_LIMIT, c.a11y_find(params)).await??;
                if out.nodes.is_empty() {
                    return done("No element matches.");
                }
                done(format!("{} matches:\n{}", out.nodes.len(), out.text))
            }
            .await,
        )
    }

    /// Runs an action.
    #[tool(
        description = "Activate an accessibility element (node id from a11y_tree or a11y_find). action \"click\" (the default) runs the element's own click/press/activate action, or with via=\"pointer\" clicks the center of its bounds with the real pointer; via=\"auto\" falls back to the pointer when the element has no action. Other action names run that action of the element."
    )]
    async fn a11y_action(&self, Parameters(p): Parameters<A11yActionParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let params = CtlAction {
                    node: p.node.clone(),
                    action: p.action.clone().unwrap_or_else(|| "click".into()),
                    via: via(p.via.as_deref())?,
                };
                let out = bounded(CALL_SLACK * 2, c.a11y_action(params)).await??;
                let summary = match (out.via, out.point) {
                    (Some(ClickVia::Pointer), Some(pt)) => format!(
                        "Clicked {} with the pointer at logical {:.0},{:.0}",
                        p.node, pt.x, pt.y
                    ),
                    _ => format!(
                        "Ran action {:?} of {}",
                        out.action.unwrap_or_default(),
                        p.node
                    ),
                };
                self.after(&c, p.screenshot, summary).await
            }
            .await,
        )
    }

    /// Replaces text.
    #[tool(
        description = "Replace the text of an editable accessibility element (node id from a11y_tree or a11y_find). It uses the element's EditableText interface, or focuses the element, selects all and types."
    )]
    async fn a11y_set_text(&self, Parameters(p): Parameters<A11ySetTextParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let params = CtlSetText {
                    node: p.node.clone(),
                    text: p.text.clone(),
                };
                let out = bounded(CALL_SLACK * 4, c.a11y_set_text(params)).await??;
                let how = match out.method {
                    TextMethod::EditableText => "through EditableText",
                    TextMethod::Typed => "by typing",
                };
                self.after(
                    &c,
                    p.screenshot,
                    format!("Set the text of {} {how}", p.node),
                )
                .await
            }
            .await,
        )
    }
}
