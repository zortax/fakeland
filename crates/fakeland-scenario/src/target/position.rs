//! Symbolic positions. A position names a coordinate frame and coordinates in it; the compositor
//! turns it into a point on an output when the event runs.

use fakeland_types::{OutputId, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::WindowRef;

/// A point in one coordinate frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Position {
    /// Logical coordinates relative to the top-left corner of an output.
    Output {
        /// The output. `None` selects the output under the pointer.
        #[serde(default)]
        output: Option<OutputId>,
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
    },
    /// Logical coordinates relative to the top-left corner of a window.
    Window {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
    },
    /// A fraction of the window size: `(0.5, 0.5)` is the center.
    WindowFraction {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// Horizontal fraction.
        fx: f64,
        /// Vertical fraction.
        fy: f64,
    },
    /// Pixel coordinates in a downscaled screenshot of an output. Logical output coordinates are
    /// the pixel coordinates times `scale`.
    Pixels {
        /// The output of the screenshot. `None` selects the output under the pointer.
        #[serde(default)]
        output: Option<OutputId>,
        /// Horizontal pixel.
        x: f64,
        /// Vertical pixel.
        y: f64,
        /// Logical units per screenshot pixel.
        scale: f64,
    },
}

impl Position {
    /// Builds a position on the output under the pointer.
    pub fn output(x: f64, y: f64) -> Self {
        Self::Output { output: None, x, y }
    }

    /// Builds a position on an output.
    pub fn on_output(output: OutputId, x: f64, y: f64) -> Self {
        Self::Output {
            output: Some(output),
            x,
            y,
        }
    }

    /// Builds a position relative to a window.
    pub fn window(window: impl Into<WindowRef>, x: f64, y: f64) -> Self {
        Self::Window {
            window: window.into(),
            x,
            y,
        }
    }

    /// Builds a fractional position in a window.
    pub fn fraction(window: impl Into<WindowRef>, fx: f64, fy: f64) -> Self {
        Self::WindowFraction {
            window: window.into(),
            fx,
            fy,
        }
    }

    /// Builds the center of a window.
    pub fn center(window: impl Into<WindowRef>) -> Self {
        Self::fraction(window, 0.5, 0.5)
    }

    /// Builds the center of a window with this id.
    pub fn center_of(id: WindowId) -> Self {
        Self::center(id)
    }

    /// Builds a position in screenshot pixels.
    pub fn pixels(x: f64, y: f64, scale: f64) -> Self {
        Self::Pixels {
            output: None,
            x,
            y,
            scale,
        }
    }

    /// Returns the coordinates in the frame of the position.
    pub fn coords(&self) -> (f64, f64) {
        match *self {
            Self::Output { x, y, .. } | Self::Window { x, y, .. } | Self::Pixels { x, y, .. } => {
                (x, y)
            }
            Self::WindowFraction { fx, fy, .. } => (fx, fy),
        }
    }

    /// Returns the position with other coordinates in the same frame.
    pub fn with_coords(&self, x: f64, y: f64) -> Self {
        let mut p = self.clone();
        match &mut p {
            Self::Output { x: px, y: py, .. }
            | Self::Window { x: px, y: py, .. }
            | Self::Pixels { x: px, y: py, .. }
            | Self::WindowFraction { fx: px, fy: py, .. } => {
                *px = x;
                *py = y;
            }
        }
        p
    }

    /// Returns the position moved by an offset in its frame.
    pub fn offset(&self, dx: f64, dy: f64) -> Self {
        let (x, y) = self.coords();
        self.with_coords(x + dx, y + dy)
    }

    /// Returns true when both positions use the same frame, so their coordinates interpolate.
    pub fn same_frame(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Output { output: a, .. }, Self::Output { output: b, .. }) => a == b,
            (Self::Window { window: a, .. }, Self::Window { window: b, .. })
            | (Self::WindowFraction { window: a, .. }, Self::WindowFraction { window: b, .. }) => {
                a == b
            }
            (
                Self::Pixels {
                    output: a,
                    scale: sa,
                    ..
                },
                Self::Pixels {
                    output: b,
                    scale: sb,
                    ..
                },
            ) => a == b && sa == sb,
            _ => false,
        }
    }

    /// Returns true when the coordinates are finite.
    pub fn is_finite(&self) -> bool {
        let (x, y) = self.coords();
        let scale_ok = match *self {
            Self::Pixels { scale, .. } => scale.is_finite() && scale > 0.0,
            _ => true,
        };
        x.is_finite() && y.is_finite() && scale_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_coords() {
        let a = Position::window(WindowRef::app_id("foot"), 10.0, 20.0);
        let b = a.with_coords(30.0, 40.0);
        assert!(a.same_frame(&b));
        assert_eq!(b.coords(), (30.0, 40.0));
        assert_eq!(a.offset(1.0, 2.0).coords(), (11.0, 22.0));
        assert!(!a.same_frame(&Position::output(10.0, 20.0)));
        assert!(!a.same_frame(&Position::window(WindowRef::Any, 0.0, 0.0)));
        assert!(Position::pixels(1.0, 1.0, 2.0).same_frame(&Position::pixels(5.0, 5.0, 2.0)));
        assert!(!Position::pixels(1.0, 1.0, 2.0).same_frame(&Position::pixels(1.0, 1.0, 1.0)));
        assert!(!Position::pixels(1.0, 1.0, 0.0).is_finite());
    }

    #[test]
    fn json_forms() {
        let p: Position = serde_json::from_str(r#"{"type":"output","x":100,"y":200}"#)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(p, Position::output(100.0, 200.0));
        let p: Position = serde_json::from_str(
            r#"{"type":"window_fraction","window":{"type":"app_id","app_id":"org.gnome.Nautilus"},"fx":0.5,"fy":0.25}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            p,
            Position::fraction(WindowRef::app_id("org.gnome.Nautilus"), 0.5, 0.25)
        );
        let p: Position = serde_json::from_str(r#"{"type":"window","x":3,"y":4}"#)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(p, Position::window(WindowRef::Focused, 3.0, 4.0));
    }
}
