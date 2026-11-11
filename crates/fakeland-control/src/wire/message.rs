//! Any message the server sends: a response or an event.

use serde::{Deserialize, Serialize};

use super::{Event, Response};

/// One line from the server. A response has an `id`; an event has an `event` object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    /// The answer to a request.
    Response(Response),
    /// An event.
    Event(Event),
}
