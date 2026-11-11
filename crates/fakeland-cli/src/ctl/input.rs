//! Input commands of ctl.

use anyhow::{Result, anyhow};
use fakeland_control::ControlClient;
use fakeland_control::methods::{
    Click, ClickParams, ComboParams, DragParams, DragTo, KeyCombo, PointerMove, PointerMoveParams,
    Scroll, ScrollParams, TypeText, TypeTextParams,
};
use fakeland_scenario::{Button, ScrollSource, TextVia, TypingCadence};

use super::args::CtlCommand;
use super::print::{emit, sent_text};

/// Parses a button name or evdev code.
pub fn parse_button(s: &str) -> Result<Button> {
    let value = match s.parse::<u32>() {
        Ok(code) => serde_json::json!(code),
        Err(_) => serde_json::json!(s.to_ascii_lowercase()),
    };
    serde_json::from_value(value).map_err(|_| anyhow!("unknown button {s:?}"))
}

/// Runs an input command. Returns `None` for commands of other groups.
pub fn run(c: &ControlClient, json: bool, cmd: &CtlCommand) -> Result<Option<i32>> {
    let sent = match cmd {
        CtlCommand::Move { x, y, frame } => c.blocking_call::<PointerMove>(PointerMoveParams {
            pos: frame.at(*x, *y)?,
        })?,
        CtlCommand::Click {
            x,
            y,
            button,
            count,
            frame,
        } => click(c, frame.at(*x, *y)?, parse_button(button)?, *count)?,
        CtlCommand::Dclick { x, y, frame } => click(c, frame.at(*x, *y)?, Button::Left, 2)?,
        CtlCommand::Rclick { x, y, frame } => click(c, frame.at(*x, *y)?, Button::Right, 1)?,
        CtlCommand::Scroll {
            x,
            y,
            dy,
            dx,
            smooth,
            finger,
            detents,
            frame,
        } => c.blocking_call::<Scroll>(ScrollParams {
            at: Some(frame.at(*x, *y)?),
            dx: *dx,
            dy: *dy,
            source: if *finger {
                ScrollSource::finger(true)
            } else {
                ScrollSource::wheel(!*detents)
            },
            smooth: *smooth,
        })?,
        CtlCommand::Drag {
            x1,
            y1,
            x2,
            y2,
            duration,
            frame,
        } => c.blocking_call::<DragTo>(DragParams {
            from: frame.at(*x1, *y1)?,
            to: frame.at(*x2, *y2)?,
            duration: *duration,
            button: Button::Left,
        })?,
        CtlCommand::Type { text, wpm, commit } => c.blocking_call::<TypeText>(TypeTextParams {
            text: text.clone(),
            cadence: TypingCadence::wpm(*wpm),
            via: if *commit {
                TextVia::TextInput
            } else {
                TextVia::Keys
            },
        })?,
        CtlCommand::Key { combo } => c.blocking_call::<KeyCombo>(ComboParams {
            combo: combo.clone(),
        })?,
        _ => return Ok(None),
    };
    emit(json, &sent, |s| sent_text(s))?;
    Ok(Some(0))
}

fn click(
    c: &ControlClient,
    pos: fakeland_scenario::Position,
    button: Button,
    count: u32,
) -> Result<Vec<fakeland::SentInput>> {
    Ok(c.blocking_call::<Click>(ClickParams {
        pos: Some(pos),
        button,
        count,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_buttons() {
        assert_eq!(parse_button("left").unwrap(), Button::Left);
        assert_eq!(parse_button("Right").unwrap(), Button::Right);
        assert_eq!(parse_button("274").unwrap(), Button::Middle);
        assert!(parse_button("thumb-wheel").is_err());
    }
}
