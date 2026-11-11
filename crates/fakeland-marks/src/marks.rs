//! The marks handle of a connection: client-level marks, the input ids that the compositor
//! announces, and the creation of per-surface marks.

use fakeland_protocol::client::fakeland_marks_manager_v1::FakelandMarksManagerV1;
use fakeland_types::InputEventId;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, EventQueue, Proxy};

use crate::check::{self, MarkError};
use crate::claims::Claims;
use crate::family::{self, Family};
use crate::input::InputMark;
use crate::queue::QueueState;
use crate::surface::{SurfaceError, SurfaceMarks};

/// The marks handle of an app connection.
///
/// The handle owns a private event queue on the app connection. The compositor sends its
/// input_event events to that queue, and the app's own queues and dispatch state stay untouched.
/// When the compositor advertises no marks global, the handle is inactive and every call is a
/// cheap no-op.
#[derive(Debug)]
pub struct Marks {
    active: Option<Active>,
}

/// The parts of an active handle.
#[derive(Debug)]
pub(crate) struct Active {
    pub(crate) conn: Connection,
    pub(crate) queue: EventQueue<QueueState>,
    pub(crate) state: QueueState,
    pub(crate) manager: FakelandMarksManagerV1,
    pub(crate) claims: Claims,
}

impl Marks {
    pub(crate) fn from_parts(active: Option<Active>) -> Self {
        Self { active }
    }

    /// Returns a handle that does nothing.
    pub fn inactive() -> Self {
        Self::from_parts(None)
    }

    /// Returns true when the compositor advertised the marks global and the handle bound it.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Records a client-level mark. The compositor timestamps the mark when it receives the
    /// request, so the call flushes the connection.
    ///
    /// The call checks the name also on an inactive handle.
    pub fn mark(&self, name: &str) -> Result<(), MarkError> {
        check::mark(name)?;
        if let Some(active) = &self.active {
            active.manager.mark(name.to_owned());
            // A failed flush keeps the request in the send buffer, and the next flush of the
            // app sends it. A dead connection surfaces in the app's own event loop.
            let _ = active.conn.flush();
        }
        Ok(())
    }

    /// Creates the marks of a surface. The marks apply on the next commit of that surface.
    ///
    /// A surface has at most one live `SurfaceMarks`. On an inactive handle the call returns an
    /// inactive `SurfaceMarks`.
    pub fn surface(&self, surface: &WlSurface) -> Result<SurfaceMarks, SurfaceError> {
        let Some(active) = &self.active else {
            return Ok(SurfaceMarks::inactive());
        };
        if !surface.is_alive() {
            return Err(SurfaceError::SurfaceDestroyed);
        }
        if !active.claims.claim(surface.id()) {
            return Err(SurfaceError::Taken);
        }
        let marks = active
            .manager
            .get_surface_marks(surface, &active.queue.handle(), ());
        Ok(SurfaceMarks::active(
            marks,
            surface.clone(),
            active.claims.clone(),
        ))
    }

    /// Returns the oldest input event that no call took yet, and removes it.
    ///
    /// The compositor sends input_event immediately before each injected input event, on the
    /// same connection and in the same flush. When the app handles an input event, the
    /// connection already read its id. Call this method once per handled input event, from the
    /// input handler, and the ids match the events in order. A pointer event group up to
    /// wl_pointer.frame counts as one input event.
    pub fn next_input(&mut self) -> Option<InputMark> {
        let active = self.active.as_mut()?;
        active.dispatch();
        active.state.inputs.pop_front()
    }

    /// Returns every input event that no call took yet, oldest first, and removes them.
    ///
    /// The method dispatches the private queue without blocking. Called from the handler of an
    /// input event, the list ends with the id of that event or with ids of events that the
    /// connection read and the app did not dispatch yet. Called after the app dispatched its
    /// pending events, for example before it renders, the list holds the ids of exactly the
    /// input events that the app handled since the last call.
    pub fn input_events(&mut self) -> Vec<InputMark> {
        match self.active.as_mut() {
            Some(active) => {
                active.dispatch();
                active.state.inputs.drain(..).collect()
            }
            None => Vec::new(),
        }
    }

    /// Returns the ids of every input event that no call took yet, oldest first, and removes
    /// them. The ordering contract is the one of `input_events`.
    pub fn take_input_ids(&mut self) -> Vec<InputEventId> {
        self.input_events().into_iter().map(|m| m.id).collect()
    }

    /// Returns the ids of an input event that a handler of `family` processes, and removes them.
    ///
    /// Call it from the input handler of the family, once per handled event or event group. The
    /// call takes the oldest id of the family and the ids of the same family that directly follow
    /// it. It drops older ids of other families, because their events came first on the
    /// connection and reached no handler. When no id of the family is queued, the call takes
    /// nothing and drops nothing.
    pub fn take_inputs_of(&mut self, family: Family) -> Vec<InputEventId> {
        match self.active.as_mut() {
            Some(active) => {
                active.dispatch();
                family::take(&mut active.state.inputs, family)
            }
            None => Vec::new(),
        }
    }
}

impl Active {
    /// Dispatches the events that the connection already read for the private queue.
    fn dispatch(&mut self) {
        // A dispatch error means a dead connection, and the app's own event loop reports it.
        let _ = self.queue.dispatch_pending(&mut self.state);
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        self.manager.destroy();
    }
}
