//! The connection to the accessibility bus. The session bus names the accessibility bus through
//! `org.a11y.Bus`; the client asks for its address and connects there.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use atspi::proxy::bus::{BusProxy, StatusProxy};
use zbus::Connection;
use zbus::proxy::MethodFlags;

use crate::{A11yError, Aliases, Node, NodeRef, TextOptions};

/// The default limit of one D-Bus call.
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// A client of the accessibility bus. It is cheap to clone, and every clone shares one
/// connection.
#[derive(Clone, Debug)]
pub struct A11y {
    pub(crate) conn: Connection,
    address: String,
    aliases: Arc<Mutex<Aliases>>,
}

impl A11y {
    /// Connects to the accessibility bus that a session bus names.
    pub async fn connect(session_bus: &str) -> Result<Self, A11yError> {
        Self::connect_with(session_bus, DEFAULT_CALL_TIMEOUT).await
    }

    /// Connects with a limit for each D-Bus call.
    pub async fn connect_with(session_bus: &str, timeout: Duration) -> Result<Self, A11yError> {
        let address = bus_address(session_bus).await?;
        Self::connect_bus(&address, timeout).await
    }

    /// Connects to an accessibility bus address.
    pub async fn connect_bus(address: &str, timeout: Duration) -> Result<Self, A11yError> {
        let conn = open(address, timeout).await?;
        Ok(Self {
            conn,
            address: address.to_string(),
            aliases: Arc::default(),
        })
    }

    /// Returns the accessibility bus address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Returns the short name of a node, for example `n12`. Clones of the client share the names.
    pub fn short_id(&self, node: &NodeRef) -> String {
        self.aliases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .alias(node)
    }

    /// Resolves a short name or a full node id.
    pub fn parse_id(&self, id: &str) -> Result<NodeRef, A11yError> {
        self.aliases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .resolve(id)
    }

    /// Renders a tree with short node names.
    pub fn render(&self, node: &Node, opts: &TextOptions) -> String {
        crate::render_with(node, opts, &mut |id| self.short_id(id))
    }

    /// Renders nodes one per line with short node names.
    pub fn render_list(&self, nodes: &[Node], opts: &TextOptions) -> String {
        crate::render_list_with(nodes, opts, &mut |id| self.short_id(id))
    }

    /// Returns the D-Bus connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Starts the AT-SPI registry through bus activation and returns once it owns its name.
    pub async fn start_registry(&self) -> Result<(), A11yError> {
        let dbus = zbus::fdo::DBusProxy::new(&self.conn).await?;
        let name = zbus::names::WellKnownName::from_static_str_unchecked(crate::id::REGISTRY);
        dbus.start_service_by_name(name, 0)
            .await
            .map_err(zbus::Error::from)?;
        Ok(())
    }
}

/// Opens a bus connection to an address.
async fn open(address: &str, timeout: Duration) -> Result<Connection, A11yError> {
    Ok(zbus::connection::Builder::address(address)?
        .method_timeout(timeout)
        .build()
        .await?)
}

/// The well-known name of the accessibility bus launcher on the session bus.
pub const BUS_NAME: &str = "org.a11y.Bus";

/// Returns the accessibility bus address that a session bus names. The call never starts a
/// launcher through bus activation: without an owner of `org.a11y.Bus` it fails with
/// [`A11yError::NoBus`].
pub async fn bus_address(session_bus: &str) -> Result<String, A11yError> {
    let no_bus = |e: &dyn std::fmt::Display| A11yError::NoBus(e.to_string());
    let conn = open(session_bus, DEFAULT_CALL_TIMEOUT)
        .await
        .map_err(|e| no_bus(&e))?;
    require_owner(&conn).await?;
    let bus = BusProxy::new(&conn).await.map_err(|e| no_bus(&e))?;
    bus.inner()
        .call_with_flags::<_, _, String>("GetAddress", MethodFlags::NoAutoStart.into(), &())
        .await
        .map_err(|e| no_bus(&e))?
        .ok_or_else(|| A11yError::NoBus("GetAddress sent no reply".into()))
}

/// Fails unless the launcher owns its name on a session bus connection.
async fn require_owner(conn: &Connection) -> Result<(), A11yError> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    let name = zbus::names::WellKnownName::from_static_str_unchecked(BUS_NAME);
    if dbus
        .name_has_owner(name.into())
        .await
        .map_err(zbus::Error::from)?
    {
        Ok(())
    } else {
        Err(A11yError::NoBus(format!("{BUS_NAME} has no owner")))
    }
}

/// The accessibility switches that `org.a11y.Status` reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusStatus {
    /// Toolkits expose their trees.
    pub enabled: bool,
    /// A screen reader runs.
    pub screen_reader: bool,
}

/// Reads `org.a11y.Status` from a session bus. Like [`bus_address`], it never starts a launcher.
pub async fn bus_status(session_bus: &str) -> Result<BusStatus, A11yError> {
    let conn = open(session_bus, DEFAULT_CALL_TIMEOUT).await?;
    require_owner(&conn).await?;
    let status = StatusProxy::builder(&conn)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    Ok(BusStatus {
        enabled: status.is_enabled().await?,
        screen_reader: status.screen_reader_enabled().await?,
    })
}
