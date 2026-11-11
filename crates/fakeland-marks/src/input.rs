//! Input ids that the compositor announces before each injected input event.

use fakeland_types::InputEventId;

pub use fakeland_protocol::client::fakeland_marks_manager_v1::InputKind;

/// One injected input event as the compositor announced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputMark {
    /// The compositor's id of the input event.
    pub id: InputEventId,
    /// The kind of the input event. It is `None` for a kind that this crate does not know.
    pub kind: Option<InputKind>,
}
