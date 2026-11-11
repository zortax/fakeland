//! A fluent builder for scenarios. Anchor and label calls apply to the next step.

use fakeland_types::{Dur, Logical, OutputId, Size};

use super::{Action, Anchor, Scenario, ScenarioError, Step};
use crate::{
    Button, CaptureTarget, ClipboardAction, Combo, Drag, Fling, Gesture, Key, KeyAction,
    OutputAction, Path, PointerAction, PointerGlide, Position, ProbeSpec, ScrollGlide,
    ScrollSource, TextVia, TouchAction, Trigger, TypingCadence, Wait, WindowAction, WindowRef,
};

/// Builds a scenario step by step.
#[derive(Clone, Debug)]
pub struct ScenarioBuilder {
    scenario: Scenario,
    anchor: Anchor,
    label: Option<String>,
    errors: Vec<ScenarioError>,
}

impl ScenarioBuilder {
    /// Starts an empty scenario.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            scenario: Scenario {
                name: name.into(),
                ..Scenario::default()
            },
            anchor: Anchor::Immediately,
            label: None,
            errors: Vec::new(),
        }
    }

    /// Starts the next step as soon as the previous one finished.
    pub fn immediately(mut self) -> Self {
        self.anchor = Anchor::Immediately;
        self
    }

    /// Starts the next step at a time since the scenario start.
    pub fn at(mut self, at: Dur) -> Self {
        self.anchor = Anchor::At { at };
        self
    }

    /// Starts the next step a time after the previous one finished.
    pub fn after(mut self, after: Dur) -> Self {
        self.anchor = Anchor::After { after };
        self
    }

    /// Starts the next step at the next vblank.
    pub fn at_next_vblank(self) -> Self {
        self.at_vblank_phase(Dur::ZERO)
    }

    /// Starts the next step a phase after the next vblank.
    pub fn at_vblank_phase(mut self, phase: Dur) -> Self {
        self.anchor = Anchor::NextVblank {
            output: None,
            phase,
        };
        self
    }

    /// Starts the next step a phase after the next vblank of an output.
    pub fn at_output_vblank(mut self, output: OutputId, phase: Dur) -> Self {
        self.anchor = Anchor::NextVblank {
            output: Some(output),
            phase,
        };
        self
    }

    /// Starts the next step when a condition holds.
    pub fn on(mut self, wait: Wait) -> Self {
        self.anchor = Anchor::OnEvent(wait);
        self
    }

    /// Names the next step.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a step with the pending anchor and label.
    pub fn step(mut self, action: impl Into<Action>) -> Self {
        let step = Step {
            anchor: std::mem::take(&mut self.anchor),
            action: action.into(),
            label: self.label.take(),
        };
        self.scenario.steps.push(step);
        self
    }

    /// Moves the pointer.
    pub fn pointer_move(self, pos: Position) -> Self {
        self.step(PointerAction::move_to(pos))
    }

    /// Presses or releases a button.
    pub fn button(self, button: Button, pressed: bool) -> Self {
        self.step(PointerAction::button(button, pressed))
    }

    /// Clicks the left button at a position.
    pub fn click(self, pos: Position) -> Self {
        self.step(PointerAction::click(Some(pos), Button::Left))
    }

    /// Clicks a button at the pointer position.
    pub fn click_here(self, button: Button) -> Self {
        self.step(PointerAction::click(None, button))
    }

    /// Double-clicks the left button at a position.
    pub fn double_click(self, pos: Position) -> Self {
        self.step(PointerAction::multi_click(Some(pos), Button::Left, 2))
    }

    /// Clicks the right button at a position.
    pub fn right_click(self, pos: Position) -> Self {
        self.step(PointerAction::click(Some(pos), Button::Right))
    }

    /// Sends one axis event.
    pub fn axis(self, dx: f64, dy: f64, source: ScrollSource) -> Self {
        self.step(PointerAction::axis(dx, dy, source))
    }

    /// Presses a key.
    pub fn key_press(self, key: impl Into<Key>) -> Self {
        self.step(KeyAction::Press { key: key.into() })
    }

    /// Releases a key.
    pub fn key_release(self, key: impl Into<Key>) -> Self {
        self.step(KeyAction::Release { key: key.into() })
    }

    /// Taps a key.
    pub fn key_tap(self, key: impl Into<Key>) -> Self {
        self.step(KeyAction::tap(key))
    }

    /// Runs a key combination such as `"ctrl+shift+t"`. A parse error fails `build`.
    pub fn combo(mut self, combo: &str) -> Self {
        match Combo::parse(combo) {
            Ok(c) => self.step(KeyAction::combo(c)),
            Err(source) => {
                let location = format!("steps[{}]", self.scenario.steps.len());
                self.errors.push(ScenarioError::Combo { location, source });
                self
            }
        }
    }

    /// Types text at the default cadence.
    pub fn type_text(self, text: impl Into<String>) -> Self {
        self.step(Action::type_text(text))
    }

    /// Types text at a cadence.
    pub fn type_with(self, text: impl Into<String>, cadence: TypingCadence) -> Self {
        self.step(Action::Type {
            text: text.into(),
            cadence,
            via: TextVia::Keys,
        })
    }

    /// Commits text through text input.
    pub fn commit_text(self, text: impl Into<String>) -> Self {
        self.step(Action::Type {
            text: text.into(),
            cadence: TypingCadence::default(),
            via: TextVia::TextInput,
        })
    }

    /// Runs a touch action.
    pub fn touch(self, action: TouchAction) -> Self {
        self.step(action)
    }

    /// Taps a touch point at a position.
    pub fn tap(self, pos: Position) -> Self {
        self.step(TouchAction::tap(pos))
    }

    /// Moves the pointer along a path.
    pub fn glide(self, glide: PointerGlide) -> Self {
        self.step(glide)
    }

    /// Moves the pointer along a straight line.
    pub fn glide_to(self, from: Position, to: Position, duration: Dur) -> Self {
        self.step(PointerGlide::new(Path::line(from, to), duration))
    }

    /// Scrolls along a velocity curve.
    pub fn scroll_glide(self, glide: ScrollGlide) -> Self {
        self.step(glide)
    }

    /// Drags with a held button.
    pub fn drag(self, drag: Drag) -> Self {
        self.step(drag)
    }

    /// Flings.
    pub fn fling(self, fling: Fling) -> Self {
        self.step(fling)
    }

    /// Runs a gesture.
    pub fn gesture(self, gesture: Gesture) -> Self {
        self.step(gesture)
    }

    /// Changes an output.
    pub fn output(self, action: OutputAction) -> Self {
        self.step(action)
    }

    /// Manages a window.
    pub fn window(self, action: WindowAction) -> Self {
        self.step(action)
    }

    /// Resizes a window.
    pub fn resize(self, window: WindowRef, size: Size<Logical>) -> Self {
        self.step(WindowAction::Resize { window, size })
    }

    /// Writes a trace mark.
    pub fn mark(self, label: impl Into<String>) -> Self {
        self.step(Action::mark(label))
    }

    /// Captures a target.
    pub fn capture(self, target: CaptureTarget) -> Self {
        self.step(Action::capture(target))
    }

    /// Captures the first output.
    pub fn screenshot(self) -> Self {
        self.capture(CaptureTarget::default())
    }

    /// Runs a probe.
    pub fn probe(self, probe: ProbeSpec) -> Self {
        self.step(probe)
    }

    /// Waits for a condition.
    pub fn wait(self, wait: Wait) -> Self {
        self.step(wait)
    }

    /// Waits for a window.
    pub fn wait_for_window(self, matcher: WindowRef) -> Self {
        self.wait(Wait::window(matcher))
    }

    /// Waits until every window settles for `quiet`.
    pub fn settle(self, quiet: Dur) -> Self {
        self.wait(Wait::settled(None, quiet))
    }

    /// Waits for `n` presented frames of any window.
    pub fn wait_frames(self, n: u32) -> Self {
        self.wait(Wait::frames(n, None))
    }

    /// Sleeps.
    pub fn sleep(self, duration: Dur) -> Self {
        self.step(Action::sleep(duration))
    }

    /// Repeats the steps that `body` adds `count` times.
    pub fn repeat(mut self, count: u32, body: impl FnOnce(Self) -> Self) -> Self {
        let inner = body(Self::new(String::new()));
        self.errors.extend(inner.errors);
        self.step(Action::Repeat {
            count,
            steps: inner.scenario.steps,
        })
    }

    /// Offers text on the clipboard.
    pub fn clipboard_text(self, text: impl Into<String>) -> Self {
        self.step(ClipboardAction::text(text))
    }

    /// Adds a trigger.
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.scenario.triggers.push(trigger);
        self
    }

    /// Sets the deadline of the whole scenario.
    pub fn timeout(mut self, timeout: Dur) -> Self {
        self.scenario.timeout = Some(timeout);
        self
    }

    /// Returns the scenario after validation.
    pub fn build(self) -> Result<Scenario, ScenarioError> {
        if let Some(e) = self.errors.into_iter().next() {
            return Err(e);
        }
        self.scenario.validate()?;
        Ok(self.scenario)
    }

    /// Returns the scenario without validation.
    pub fn build_unchecked(self) -> Scenario {
        self.scenario
    }
}
