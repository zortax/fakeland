//! Fetching one node: the Accessible properties in one concurrent batch, then the extents,
//! actions, text and value of the interfaces that the node implements in a second batch.

use atspi::CoordType;
use atspi::proxy::action::ActionProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::text::TextProxy;
use atspi::proxy::value::ValueProxy;
use futures_util::future::{OptionFuture, join, join_all};
use futures_util::join;
use zbus::Connection;

use super::proxy::{accessible, node_ref, proxy, role_name};
use crate::{CoordSpace, Extents, Node, NodeRef, NodeValue, states};

/// The character that stands for an embedded child in text contents.
const OBJECT_REPLACEMENT: char = '\u{fffc}';

/// The prefix of AT-SPI interface names.
const INTERFACE_PREFIX: &str = "org.a11y.atspi.";

/// What a fetch reads besides the Accessible properties.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FetchSpec {
    /// The coordinate space of the extents.
    pub space: CoordSpace,
    /// The most characters of text contents. Zero skips text.
    pub text_limit: usize,
    /// The fetch lists the children.
    pub children: bool,
}

/// A fetched node and the references of its children.
pub(crate) struct Fetched {
    pub node: Node,
    pub children: Vec<NodeRef>,
}

/// Fetches one node. A node that does not answer comes back with its error set.
pub(crate) async fn fetch(conn: &Connection, id: NodeRef, spec: FetchSpec) -> Fetched {
    match try_fetch(conn, &id, spec).await {
        Ok(f) => f,
        Err(e) => {
            let mut node = Node::empty(id);
            node.error = Some(e.to_string());
            Fetched {
                node,
                children: Vec::new(),
            }
        }
    }
}

async fn try_fetch(conn: &Connection, id: &NodeRef, spec: FetchSpec) -> zbus::Result<Fetched> {
    let acc = accessible(conn, id).await?;
    let children = OptionFuture::from(spec.children.then(|| acc.get_children()));
    let (role, name, description, state, interfaces, count, children) = join!(
        role_name(&acc),
        acc.name(),
        acc.description(),
        acc.get_state(),
        acc.get_interfaces(),
        acc.child_count(),
        children,
    );
    let mut node = Node::empty(id.clone());
    node.role = role?;
    node.name = name.unwrap_or_default();
    node.description = description.unwrap_or_default();
    node.states = states::state_names(&state.unwrap_or_default());
    node.interfaces = interfaces
        .unwrap_or_default()
        .into_iter()
        .map(|i| i.strip_prefix(INTERFACE_PREFIX).unwrap_or(&i).to_string())
        .collect();
    node.child_count = count.unwrap_or(0).max(0) as usize;
    let children: Vec<NodeRef> = match children {
        Some(list) => list
            .unwrap_or_default()
            .into_iter()
            .map(node_ref)
            .filter(|c| !c.is_null())
            .collect(),
        None => Vec::new(),
    };

    let has = |i: &str| node.implements(i);
    let (extents, actions) = join(
        OptionFuture::from(has("Component").then(|| extents(conn, id, spec.space))),
        OptionFuture::from(has("Action").then(|| actions(conn, id))),
    )
    .await;
    let (text, value) = join(
        OptionFuture::from(
            (has("Text") && spec.text_limit > 0).then(|| text(conn, id, spec.text_limit)),
        ),
        OptionFuture::from(has("Value").then(|| value(conn, id))),
    )
    .await;
    node.extents = extents.flatten();
    node.actions = actions.flatten().unwrap_or_default();
    node.text = text.flatten();
    node.value = value.flatten();
    Ok(Fetched { node, children })
}

/// Reads the extents in a space. A toolkit that rejects window coordinates answers in screen
/// coordinates.
pub(crate) async fn extents(conn: &Connection, id: &NodeRef, space: CoordSpace) -> Option<Extents> {
    let c: ComponentProxy<'_> = proxy(conn, id).await.ok()?;
    let read = |s: CoordSpace| {
        let c = c.clone();
        async move {
            let t = match s {
                CoordSpace::Window => CoordType::Window,
                CoordSpace::Screen => CoordType::Screen,
            };
            c.get_extents(t)
                .await
                .ok()
                .map(|(x, y, w, h)| Extents {
                    x,
                    y,
                    w,
                    h,
                    space: s,
                })
                .filter(Extents::is_plausible)
        }
    };
    match read(space).await {
        Some(e) => Some(e),
        None if space == CoordSpace::Window => read(CoordSpace::Screen).await,
        None => None,
    }
}

/// Reads the action names. `GetActions` carries localized names, which some toolkits leave
/// empty, so each name comes from `GetName`.
pub(crate) async fn actions(conn: &Connection, id: &NodeRef) -> Option<Vec<String>> {
    let a: ActionProxy<'_> = proxy(conn, id).await.ok()?;
    let n = a.n_actions().await.ok()?.max(0);
    let names = join_all((0..n).map(|i| a.get_name(i))).await;
    Some(names.into_iter().map(Result::unwrap_or_default).collect())
}

async fn text(conn: &Connection, id: &NodeRef, limit: usize) -> Option<String> {
    let t: TextProxy<'_> = proxy(conn, id).await.ok()?;
    let count = t.character_count().await.ok()?.max(0) as usize;
    if count == 0 {
        return Some(String::new());
    }
    let end = count.min(limit) as i32;
    let text = t.get_text(0, end).await.ok()?;
    Some(text.replace(OBJECT_REPLACEMENT, ""))
}

async fn value(conn: &Connection, id: &NodeRef) -> Option<NodeValue> {
    let v: ValueProxy<'_> = proxy(conn, id).await.ok()?;
    let (current, min, max) = join!(v.current_value(), v.minimum_value(), v.maximum_value());
    Some(NodeValue {
        current: current.ok()?,
        min: min.ok(),
        max: max.ok(),
    })
}
