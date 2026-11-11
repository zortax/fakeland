//! Text through `zwp_text_input_v3`: `commit_string` and `done` on the enabled text input of the
//! focused client. Text without an enabled text input types as keys.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_scenario::Key;
use fakeland_trace::{InputKind, InputSource};

use super::{InjectError, PendingSent, SentInput};
use crate::server::State;

impl State {
    /// Returns true when the focused client enabled a text input.
    pub fn text_input_active(&self) -> bool {
        self.text_inputs.active().is_some()
    }

    /// Commits text through the enabled text input. Returns no record without one.
    pub(super) fn inject_text_input(&mut self, text: &str, source: InputSource) -> Vec<SentInput> {
        let Some(surface) = self.text_inputs.active().map(|(_, s)| s.clone()) else {
            return Vec::new();
        };
        let (id, sid) = self.mark_input(Some(&surface), MarkKind::Text);
        self.text_inputs.commit_string(text);
        self.record_sent(vec![PendingSent {
            id,
            kind: InputKind::Text,
            surface: sid,
            pos: None,
            detail: text.to_string(),
            source,
        }])
    }

    /// Sends text: through the enabled text input, or as a press and release per character.
    pub(super) fn inject_text(
        &mut self,
        text: &str,
        source: InputSource,
    ) -> Result<Vec<SentInput>, InjectError> {
        if self.text_input_active() {
            return Ok(self.inject_text_input(text, source));
        }
        let mut sent = Vec::new();
        for c in text.chars() {
            let key = Key::char(c);
            sent.extend(self.inject_key(&key, true, source)?);
            sent.extend(self.inject_key(&key, false, source)?);
        }
        Ok(sent)
    }
}
