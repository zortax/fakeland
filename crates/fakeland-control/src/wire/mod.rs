//! The wire format of the control socket. Each message is one JSON object on one line. A client
//! sends requests; the server sends responses and events.

mod codec;
mod error;
mod event;
mod message;
mod request;
mod response;
mod version;

pub use codec::{LineReader, MAX_LINE, encode_line};
pub use error::{ErrorBody, ErrorCode};
pub use event::{Event, EventBody};
pub use message::ServerMessage;
pub use request::Request;
pub use response::{Response, ResponseBody};
pub use version::PROTOCOL_VERSION;
