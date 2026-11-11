//! D-Bus proxies of accessible objects. The Accessible proxy here reads states, interfaces and
//! object references as raw values, so a toolkit that sends a state bit or an interface name
//! unknown to `atspi` still answers. The other interfaces use the `atspi` proxies.

use zbus::Connection;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;

use crate::NodeRef;

/// The `org.a11y.atspi.Accessible` interface with raw return types.
#[zbus::proxy(interface = "org.a11y.atspi.Accessible", gen_blocking = false)]
pub(crate) trait RawAccessible {
    /// The children as bus name and path pairs.
    fn get_children(&self) -> zbus::Result<Vec<(String, OwnedObjectPath)>>;
    /// The two state words.
    fn get_state(&self) -> zbus::Result<Vec<u32>>;
    /// The interface names.
    fn get_interfaces(&self) -> zbus::Result<Vec<String>>;
    /// The role name.
    fn get_role_name(&self) -> zbus::Result<String>;
    /// The role number.
    fn get_role(&self) -> zbus::Result<u32>;
    /// The accessible name.
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    /// The accessible description.
    #[zbus(property)]
    fn description(&self) -> zbus::Result<String>;
    /// The number of children.
    #[zbus(property)]
    fn child_count(&self) -> zbus::Result<i32>;
    /// The parent as a bus name and path pair.
    #[zbus(property)]
    fn parent(&self) -> zbus::Result<(String, OwnedObjectPath)>;
}

/// Builds a proxy of one interface for a node without a property cache.
pub(crate) async fn proxy<'a, P>(conn: &Connection, id: &'a NodeRef) -> zbus::Result<P>
where
    P: zbus::proxy::Defaults + From<zbus::Proxy<'a>>,
{
    zbus::proxy::Builder::<P>::new(conn)
        .destination(id.bus.as_str())?
        .path(id.path.as_str())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// Builds the raw Accessible proxy for a node.
pub(crate) async fn accessible<'a>(
    conn: &Connection,
    id: &'a NodeRef,
) -> zbus::Result<RawAccessibleProxy<'a>> {
    proxy(conn, id).await
}

/// Turns a raw object reference into a node id.
pub(crate) fn node_ref((bus, path): (String, OwnedObjectPath)) -> NodeRef {
    NodeRef::new(bus, path.as_str())
}

/// Reads the role name. Toolkits without `GetRoleName`, such as AccessKit, answer `GetRole`,
/// whose number maps to the AT-SPI role name.
pub(crate) async fn role_name(acc: &RawAccessibleProxy<'_>) -> zbus::Result<String> {
    match acc.get_role_name().await {
        Ok(name) => Ok(name),
        Err(named) => match acc.get_role().await {
            Ok(n) => Ok(crate::roles::name_of(n)),
            Err(_) => Err(named),
        },
    }
}
