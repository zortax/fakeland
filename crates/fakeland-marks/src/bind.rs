//! Construction of a `Marks` handle from the globals of an app connection. A missing global
//! gives an inactive handle.

use fakeland_protocol::MANAGER_VERSION;
use fakeland_protocol::client::fakeland_marks_manager_v1::FakelandMarksManagerV1;
use wayland_client::globals::{GlobalError, GlobalList, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, EventQueue};

use crate::claims::Claims;
use crate::marks::{Active, Marks};
use crate::queue::QueueState;

/// A failure to read the globals of a connection.
#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    /// The registry roundtrip failed.
    #[error("cannot read the globals: {0}")]
    Global(#[from] GlobalError),
}

impl Marks {
    /// Binds the marks global from a global list of the app. The handle is inactive when the
    /// list holds no marks global.
    ///
    /// The global list can belong to any event queue of the app. The marks objects go to a new
    /// private queue.
    pub fn bind(conn: &Connection, globals: &GlobalList) -> Self {
        let queue = conn.new_event_queue();
        let manager = globals.bind::<FakelandMarksManagerV1, QueueState, ()>(
            &queue.handle(),
            1..=MANAGER_VERSION,
            (),
        );
        Self::from_parts(manager.ok().map(|m| active(conn, queue, m)))
    }

    /// Binds the marks global from a wl_registry.global event that the app received. Use it
    /// when the app reads the registry itself and the interface name equals
    /// `MANAGER_INTERFACE`.
    pub fn from_global(conn: &Connection, registry: &WlRegistry, name: u32, version: u32) -> Self {
        let version = version.min(MANAGER_VERSION);
        if version == 0 {
            return Self::inactive();
        }
        let queue = conn.new_event_queue();
        let manager = registry.bind::<FakelandMarksManagerV1, (), QueueState>(
            name,
            version,
            &queue.handle(),
            (),
        );
        Self::from_parts(Some(active(conn, queue, manager)))
    }

    /// Reads the globals on a private registry and binds the marks global. The call makes one
    /// blocking roundtrip on a private queue and leaves the app's queues untouched.
    pub fn discover(conn: &Connection) -> Result<Self, DiscoverError> {
        let (globals, queue) = registry_queue_init::<QueueState>(conn)?;
        let manager = globals.bind::<FakelandMarksManagerV1, QueueState, ()>(
            &queue.handle(),
            1..=MANAGER_VERSION,
            (),
        );
        Ok(Self::from_parts(
            manager.ok().map(|m| active(conn, queue, m)),
        ))
    }
}

fn active(
    conn: &Connection,
    queue: EventQueue<QueueState>,
    manager: FakelandMarksManagerV1,
) -> Active {
    Active {
        conn: conn.clone(),
        queue,
        state: QueueState::default(),
        manager,
        claims: Claims::default(),
    }
}
