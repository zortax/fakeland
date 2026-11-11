//! Input operations.

use fakeland::{ActionOutcome, SentInput};
use fakeland_scenario::{
    Action, Combo, KeyAction, PointerAction, ScrollAxis, ScrollGlide, TouchAction,
};

use super::capture::shot_info;
use super::{Controller, OpError};
use crate::methods::{
    ActOutcome, ActParams, ClickParams, ComboParams, DragParams, InjectParams, KeyParams,
    PointerMoveParams, ScrollParams, TouchTapParams, TypeTextParams,
};

/// Converts an action outcome into its wire form. A capture carries its PNG inline.
pub fn act_outcome(outcome: ActionOutcome) -> Result<ActOutcome, OpError> {
    Ok(match outcome {
        ActionOutcome::Done => ActOutcome::Done,
        ActionOutcome::Input { sent } => ActOutcome::Input { sent },
        ActionOutcome::Captured { label, screenshot } => ActOutcome::Captured {
            label,
            shot: Box::new(shot_info(&screenshot, None, true)?),
        },
        ActionOutcome::Probe(report) => ActOutcome::Probe {
            report: Box::new(report),
        },
        ActionOutcome::Waited(output) => ActOutcome::Waited {
            output: Box::new(output),
        },
    })
}

impl Controller {
    /// Sends primitives at once.
    pub async fn inject(&self, p: InjectParams) -> Result<Vec<SentInput>, OpError> {
        Ok(self.session().inject(p.primitives).await?)
    }

    /// Runs one action.
    pub async fn act(&self, p: ActParams) -> Result<ActOutcome, OpError> {
        act_outcome(self.session().act(p.action).await?)
    }

    async fn act_input(&self, action: Action) -> Result<Vec<SentInput>, OpError> {
        match self.session().act(action).await? {
            ActionOutcome::Input { sent } => Ok(sent),
            _ => Ok(Vec::new()),
        }
    }

    /// Moves the pointer.
    pub async fn pointer_move(&self, p: PointerMoveParams) -> Result<Vec<SentInput>, OpError> {
        self.act_input(PointerAction::move_to(p.pos).into()).await
    }

    /// Clicks a button `count` times.
    pub async fn click(&self, p: ClickParams) -> Result<Vec<SentInput>, OpError> {
        if p.count == 0 {
            return Err(OpError::InvalidParams("count must be at least 1".into()));
        }
        self.act_input(PointerAction::multi_click(p.pos, p.button, p.count).into())
            .await
    }

    /// Scrolls by a distance, as one event or spread over a time.
    pub async fn scroll(&self, p: ScrollParams) -> Result<Vec<SentInput>, OpError> {
        let Some(duration) = p.smooth else {
            let mut sent = Vec::new();
            if let Some(pos) = p.at {
                sent.extend(self.act_input(PointerAction::move_to(pos).into()).await?);
            }
            sent.extend(
                self.act_input(PointerAction::axis(p.dx, p.dy, p.source).into())
                    .await?,
            );
            return Ok(sent);
        };
        let secs = duration.0.as_secs_f64();
        if secs <= 0.0 {
            return Err(OpError::InvalidParams(
                "smooth needs a positive time".into(),
            ));
        }
        let mut sent = Vec::new();
        let mut at = p.at;
        for (axis, distance) in [(ScrollAxis::Vertical, p.dy), (ScrollAxis::Horizontal, p.dx)] {
            if distance == 0.0 {
                continue;
            }
            let mut glide = ScrollGlide::constant(distance / secs, duration)
                .axis(axis)
                .source(p.source);
            if let Some(pos) = at.take() {
                glide = glide.at(pos);
            }
            sent.extend(self.act_input(glide.into()).await?);
        }
        Ok(sent)
    }

    /// Drags along a line.
    pub async fn drag(&self, p: DragParams) -> Result<Vec<SentInput>, OpError> {
        let mut drag = fakeland_scenario::Drag::line(p.from, p.to, p.duration).button(p.button);
        if let Some(rate) = self.session().config().input.pointer.report_rate {
            drag.rate_hz = rate.0;
        }
        self.act_input(drag.into()).await
    }

    /// Taps a key.
    pub async fn key(&self, p: KeyParams) -> Result<Vec<SentInput>, OpError> {
        self.act_input(KeyAction::tap(p.key).into()).await
    }

    /// Presses a key combination.
    pub async fn combo(&self, p: ComboParams) -> Result<Vec<SentInput>, OpError> {
        let combo = Combo::parse(&p.combo)
            .map_err(|e| OpError::InvalidParams(format!("{}: {e}", p.combo)))?;
        self.act_input(KeyAction::combo(combo).into()).await
    }

    /// Types text.
    pub async fn type_text(&self, p: TypeTextParams) -> Result<Vec<SentInput>, OpError> {
        self.act_input(Action::Type {
            text: p.text,
            cadence: p.cadence,
            via: p.via,
        })
        .await
    }

    /// Taps a touch point.
    pub async fn touch_tap(&self, p: TouchTapParams) -> Result<Vec<SentInput>, OpError> {
        self.act_input(TouchAction::tap(p.pos).into()).await
    }
}
