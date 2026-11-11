//! The accessible apps below the registry root: their names, pids, toolkits and frames, and the
//! pairing of frames with compositor windows.

use std::collections::HashMap;

use atspi::proxy::application::ApplicationProxy;
use fakeland_types::{ProcessId, WindowId};
use futures_util::future::join_all;
use futures_util::join;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::A11y;
use super::proxy::{accessible, node_ref, proxy, role_name};
use crate::matching::{FrameCandidate, match_frames};
use crate::{A11yError, NodeRef, WindowFrame, states};

/// One accessible app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AppInfo {
    /// The root node of the app.
    pub root: NodeRef,
    /// The app name.
    pub name: String,
    /// The pid of the bus connection.
    pub pid: Option<ProcessId>,
    /// The toolkit name, for example `GTK` or `Chromium`.
    pub toolkit: String,
    /// The toplevel nodes of the app.
    pub frames: Vec<FrameInfo>,
}

/// A toplevel node of an app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrameInfo {
    /// The node.
    pub node: NodeRef,
    /// The role name.
    pub role: String,
    /// The accessible name.
    pub name: String,
    /// The set states.
    pub states: Vec<String>,
    /// The compositor window of the frame.
    pub window: Option<WindowId>,
}

impl A11y {
    /// Lists the accessible apps and pairs their frames with compositor windows.
    pub async fn apps(&self, windows: &[WindowFrame]) -> Result<Vec<AppInfo>, A11yError> {
        let roots = self.app_roots().await?;
        let apps = join_all(roots.into_iter().map(|r| self.app_info(r, windows))).await;
        Ok(apps.into_iter().flatten().collect())
    }

    /// Returns the root references of every registered app.
    pub(crate) async fn app_roots(&self) -> Result<Vec<NodeRef>, A11yError> {
        let root = NodeRef::registry_root();
        let acc = accessible(&self.conn, &root).await?;
        Ok(acc
            .get_children()
            .await?
            .into_iter()
            .map(node_ref)
            .filter(|n| !n.is_null())
            .collect())
    }

    /// Returns the pid behind a bus name.
    pub(crate) async fn pid_of(&self, bus: &str) -> Option<ProcessId> {
        let dbus = zbus::fdo::DBusProxy::new(&self.conn).await.ok()?;
        let name = zbus::names::BusName::try_from(bus).ok()?;
        dbus.get_connection_unix_process_id(name)
            .await
            .ok()
            .map(ProcessId)
    }

    async fn app_info(&self, root: NodeRef, windows: &[WindowFrame]) -> Option<AppInfo> {
        let acc = accessible(&self.conn, &root).await.ok()?;
        let app: Option<ApplicationProxy<'_>> = proxy(&self.conn, &root).await.ok();
        let toolkit = async {
            match &app {
                Some(a) => a.toolkit_name().await.unwrap_or_default(),
                None => String::new(),
            }
        };
        let (name, children, pid, toolkit) = join!(
            acc.name(),
            acc.get_children(),
            self.pid_of(&root.bus),
            toolkit
        );
        let children: Vec<NodeRef> = children
            .unwrap_or_default()
            .into_iter()
            .map(node_ref)
            .filter(|n| !n.is_null())
            .collect();
        let frames = self.frames(pid, children, windows).await;
        Some(AppInfo {
            root,
            name: name.unwrap_or_default(),
            pid,
            toolkit,
            frames,
        })
    }

    /// Reads the toplevel nodes of an app and pairs them with windows.
    pub(crate) async fn frames(
        &self,
        pid: Option<ProcessId>,
        children: Vec<NodeRef>,
        windows: &[WindowFrame],
    ) -> Vec<FrameInfo> {
        let mut frames: Vec<FrameInfo> = join_all(children.into_iter().map(|c| self.frame(c)))
            .await
            .into_iter()
            .collect();
        let candidates: Vec<FrameCandidate> = frames
            .iter()
            .map(|f| FrameCandidate {
                name: f.name.clone(),
                active: states::has_state(&f.states, "active"),
                showing: states::has_state(&f.states, "showing"),
            })
            .collect();
        let pairs = match pid {
            Some(p) => match_frames(Some(p), &candidates, windows),
            None => vec![None; frames.len()],
        };
        for (f, w) in frames.iter_mut().zip(pairs) {
            f.window = w.map(|i| windows[i].id);
        }
        frames
    }

    async fn frame(&self, node: NodeRef) -> FrameInfo {
        let read = async {
            let acc = accessible(&self.conn, &node).await.ok()?;
            let (role, name, state) = join!(role_name(&acc), acc.name(), acc.get_state());
            Some((
                role.unwrap_or_default(),
                name.unwrap_or_default(),
                states::state_names(&state.unwrap_or_default()),
            ))
        };
        let (role, name, states) = read.await.unwrap_or_default();
        FrameInfo {
            node,
            role,
            name,
            states,
            window: None,
        }
    }

    /// Returns the frame of a window, its app pid and the app root.
    pub(crate) async fn window_frame(
        &self,
        window: &WindowFrame,
        windows: &[WindowFrame],
    ) -> Result<(NodeRef, ProcessId), A11yError> {
        let roots = self.app_roots().await?;
        let pids = join_all(roots.iter().map(|r| self.pid_of(&r.bus))).await;
        let mut by_pid: HashMap<ProcessId, Vec<NodeRef>> = HashMap::new();
        for (r, p) in roots.into_iter().zip(pids) {
            if let Some(p) = p.filter(|&p| window.owned_by(p)) {
                by_pid.entry(p).or_default().push(r);
            }
        }
        if by_pid.is_empty() {
            return Err(A11yError::NoApp(format!(
                "the process of window {}",
                window.id
            )));
        }
        for (pid, roots) in by_pid {
            for root in roots {
                let acc = accessible(&self.conn, &root).await?;
                let children = acc
                    .get_children()
                    .await?
                    .into_iter()
                    .map(node_ref)
                    .filter(|n| !n.is_null())
                    .collect();
                let frames = self.frames(Some(pid), children, windows).await;
                if let Some(f) = frames.into_iter().find(|f| f.window == Some(window.id)) {
                    return Ok((f.node, pid));
                }
            }
        }
        Err(A11yError::NoFrame(window.id.to_string()))
    }
}
