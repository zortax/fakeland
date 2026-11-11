//! The log form of decoded input effects.

use serde::Serialize;

use crate::input::{AxisFrame, InputEffect};

/// One input effect as a log object, tagged by `type`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputDetail {
    /// The pointer entered the surface.
    Enter {
        /// Surface-local logical x.
        x: f64,
        /// Surface-local logical y.
        y: f64,
    },
    /// The pointer moved.
    Motion {
        /// Surface-local logical x.
        x: f64,
        /// Surface-local logical y.
        y: f64,
    },
    /// The pointer left the surface.
    Leave,
    /// A button changed state.
    Button {
        /// The evdev button code.
        button: u32,
        /// True for a press.
        pressed: bool,
        /// The pointer position `[x, y]`, when the pointer is on the surface.
        #[serde(skip_serializing_if = "Option::is_none")]
        pos: Option<[f64; 2]>,
    },
    /// A scroll frame.
    Axis(AxisFrame),
    /// A key changed state.
    Key {
        /// The evdev key code.
        code: u32,
        /// The xkb keysym.
        keysym: u32,
        /// The xkb keysym name.
        name: String,
        /// The produced text.
        utf8: String,
        /// True for a press.
        pressed: bool,
    },
    /// Committed input method text.
    Text {
        /// The text.
        text: String,
    },
    /// A touch point went down.
    TouchDown {
        /// The touch point id.
        id: i32,
        /// Surface-local logical x.
        x: f64,
        /// Surface-local logical y.
        y: f64,
    },
    /// A touch point moved.
    TouchMotion {
        /// The touch point id.
        id: i32,
        /// Surface-local logical x.
        x: f64,
        /// Surface-local logical y.
        y: f64,
    },
    /// A touch point went up.
    TouchUp {
        /// The touch point id.
        id: i32,
    },
    /// Every touch point was cancelled.
    TouchCancel,
}

impl InputDetail {
    /// Builds the log form of an effect. `keysym_name` names a keysym.
    pub fn of(effect: &InputEffect, keysym_name: impl Fn(u32) -> String) -> Self {
        match effect {
            InputEffect::PointerEnter { pos } => Self::Enter { x: pos.x, y: pos.y },
            InputEffect::PointerMotion { pos } => Self::Motion { x: pos.x, y: pos.y },
            InputEffect::PointerLeave => Self::Leave,
            InputEffect::Button {
                button,
                pressed,
                pos,
            } => Self::Button {
                button: *button,
                pressed: *pressed,
                pos: pos.map(|p| [p.x, p.y]),
            },
            InputEffect::Axis(a) => Self::Axis(*a),
            InputEffect::Key {
                code,
                keysym,
                utf8,
                pressed,
            } => Self::Key {
                code: *code,
                keysym: *keysym,
                name: keysym_name(*keysym),
                utf8: utf8.clone(),
                pressed: *pressed,
            },
            InputEffect::Text { text } => Self::Text { text: text.clone() },
            InputEffect::TouchDown { id, pos } => Self::TouchDown {
                id: *id,
                x: pos.x,
                y: pos.y,
            },
            InputEffect::TouchMotion { id, pos } => Self::TouchMotion {
                id: *id,
                x: pos.x,
                y: pos.y,
            },
            InputEffect::TouchUp { id } => Self::TouchUp { id: *id },
            InputEffect::TouchCancel => Self::TouchCancel,
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Point;

    use super::*;
    use crate::input::AxisSource;

    #[test]
    fn serializes_tagged() {
        let d = InputDetail::of(
            &InputEffect::PointerMotion {
                pos: Point::new(1.5, 2.0),
            },
            |_| String::new(),
        );
        assert_eq!(
            serde_json::to_string(&d).unwrap(),
            r#"{"type":"motion","x":1.5,"y":2.0}"#
        );
        let axis = InputDetail::of(
            &InputEffect::Axis(AxisFrame {
                source: Some(AxisSource::Wheel),
                dy: 15.0,
                value120_y: 120,
                ..AxisFrame::default()
            }),
            |_| String::new(),
        );
        let v: serde_json::Value = serde_json::to_value(&axis).unwrap();
        assert_eq!(v["type"], "axis");
        assert_eq!(v["source"], "wheel");
        assert_eq!(v["value120_y"], 120);
        assert_eq!(v["dy"], 15.0);
    }

    #[test]
    fn keys_carry_the_name() {
        let d = InputDetail::of(
            &InputEffect::Key {
                code: 30,
                keysym: 0x61,
                utf8: "a".into(),
                pressed: true,
            },
            |k| format!("sym{k:x}"),
        );
        let v: serde_json::Value = serde_json::to_value(&d).unwrap();
        assert_eq!(v["name"], "sym61");
        assert_eq!(v["pressed"], true);
    }
}
