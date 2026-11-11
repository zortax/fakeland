//! Input calls of the session: raw primitives, single actions with their outcome, and
//! convenience methods for pointer, keyboard, text and touch. Programs with timing, such as
//! typing or drags, run on the compositor thread; the call returns when the last event went out.

use fakeland_scenario::{
    Action, Button, Combo, Drag, Key, KeyAction, PointerAction, Position, Primitive, ScrollSource,
    TextVia, TouchAction, TypingCadence,
};
use fakeland_trace::InputSource;
use fakeland_types::Dur;

use super::{Session, SessionError};
use crate::actions::{ActionCtx, ActionOutcome};
use crate::input::SentInput;
use crate::util::block_on;

impl Session {
    /// Injects primitives at once and closes their pointer and touch groups. Returns the events
    /// that reached clients, with send times after the flush. With a pointer report rate, a
    /// motion within one report period of the last one goes out at its report slot, and only the
    /// trace records it. [`Session::act`] returns such a motion too.
    pub async fn inject(&self, primitives: Vec<Primitive>) -> Result<Vec<SentInput>, SessionError> {
        Ok(self
            .inner
            .call(move |state| state.inject_batch(&primitives, InputSource::Api))
            .await??)
    }

    /// Injects primitives on the calling thread.
    pub fn blocking_inject(
        &self,
        primitives: Vec<Primitive>,
    ) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.inject(primitives))
    }

    /// Runs one scenario action on the compositor thread and returns its outcome. A repeat runs
    /// only inside a scenario.
    pub async fn act(&self, action: Action) -> Result<ActionOutcome, SessionError> {
        self.inner
            .call_with(move |state, reply| {
                state.perform(
                    &action,
                    ActionCtx::new(InputSource::Api),
                    Box::new(move |_, result| {
                        let _ = reply.send(result.map_err(SessionError::from));
                    }),
                );
            })
            .await?
    }

    /// Runs one action on the calling thread.
    pub fn blocking_act(&self, action: Action) -> Result<ActionOutcome, SessionError> {
        block_on(self.act(action))
    }

    async fn act_input(&self, action: Action) -> Result<Vec<SentInput>, SessionError> {
        match self.act(action).await? {
            ActionOutcome::Input { sent } => Ok(sent),
            _ => Ok(Vec::new()),
        }
    }

    /// Moves the pointer to a position.
    pub async fn pointer_move(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(PointerAction::move_to(pos).into()).await
    }

    /// Moves the pointer on the calling thread.
    pub fn blocking_pointer_move(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.pointer_move(pos))
    }

    /// Moves to a position and clicks a button.
    pub async fn click(
        &self,
        pos: Position,
        button: Button,
    ) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(PointerAction::click(Some(pos), button).into())
            .await
    }

    /// Clicks on the calling thread.
    pub fn blocking_click(
        &self,
        pos: Position,
        button: Button,
    ) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.click(pos, button))
    }

    /// Moves to a position and double-clicks the left button.
    pub async fn double_click(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(PointerAction::multi_click(Some(pos), Button::Left, 2).into())
            .await
    }

    /// Double-clicks on the calling thread.
    pub fn blocking_double_click(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.double_click(pos))
    }

    /// Sends one axis event at the pointer, or at `at` after a move there. A wheel source sends
    /// `value120` steps of its detent.
    pub async fn scroll(
        &self,
        at: Option<Position>,
        dx: f64,
        dy: f64,
        source: ScrollSource,
    ) -> Result<Vec<SentInput>, SessionError> {
        let mut sent = Vec::new();
        if let Some(pos) = at {
            sent.extend(self.pointer_move(pos).await?);
        }
        sent.extend(
            self.act_input(PointerAction::axis(dx, dy, source).into())
                .await?,
        );
        Ok(sent)
    }

    /// Scrolls on the calling thread.
    pub fn blocking_scroll(
        &self,
        at: Option<Position>,
        dx: f64,
        dy: f64,
        source: ScrollSource,
    ) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.scroll(at, dx, dy, source))
    }

    /// Drags with the left button along a line. Motion events follow the pointer report rate,
    /// or 125 Hz without one.
    pub async fn drag(
        &self,
        from: Position,
        to: Position,
        duration: Dur,
    ) -> Result<Vec<SentInput>, SessionError> {
        let mut drag = Drag::line(from, to, duration);
        if let Some(rate) = self.config().input.pointer.report_rate {
            drag.rate_hz = rate.0;
        }
        self.act_input(drag.into()).await
    }

    /// Drags on the calling thread.
    pub fn blocking_drag(
        &self,
        from: Position,
        to: Position,
        duration: Dur,
    ) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.drag(from, to, duration))
    }

    /// Taps a key.
    pub async fn key(&self, key: impl Into<Key>) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(KeyAction::tap(key).into()).await
    }

    /// Taps a key on the calling thread.
    pub fn blocking_key(&self, key: impl Into<Key>) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.key(key))
    }

    /// Presses a key combination such as `ctrl+shift+t`.
    pub async fn combo(&self, combo: &str) -> Result<Vec<SentInput>, SessionError> {
        let combo = Combo::parse(combo)?;
        self.act_input(KeyAction::combo(combo).into()).await
    }

    /// Presses a key combination on the calling thread.
    pub fn blocking_combo(&self, combo: &str) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.combo(combo))
    }

    /// Types text at a cadence. Characters the keymap lacks go through text input or a
    /// temporary keymap.
    pub async fn type_text(
        &self,
        text: impl Into<String>,
        cadence: TypingCadence,
    ) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(Action::Type {
            text: text.into(),
            cadence,
            via: TextVia::Keys,
        })
        .await
    }

    /// Types text on the calling thread.
    pub fn blocking_type_text(
        &self,
        text: impl Into<String>,
        cadence: TypingCadence,
    ) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.type_text(text, cadence))
    }

    /// Taps a touch point at a position.
    pub async fn touch_tap(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        self.act_input(TouchAction::tap(pos).into()).await
    }

    /// Taps a touch point on the calling thread.
    pub fn blocking_touch_tap(&self, pos: Position) -> Result<Vec<SentInput>, SessionError> {
        block_on(self.touch_tap(pos))
    }
}
