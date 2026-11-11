//! Coordinate flags: where `x y` arguments live.

use clap::Args;
use fakeland_control::spec::parse_window_ref;
use fakeland_scenario::Position;
use fakeland_types::OutputId;

/// The frame of `x y` coordinates. The default is logical coordinates on the output under the
/// pointer.
#[derive(Debug, Clone, Default, Args)]
pub struct Frame {
    /// Coordinates are relative to this window: focused, any, ID, app:APP_ID or title:REGEX.
    #[arg(long, value_name = "WINDOW")]
    pub window: Option<String>,
    /// Coordinates are on this output id.
    #[arg(long, value_name = "ID")]
    pub output: Option<u32>,
    /// Coordinates are pixels of a screenshot with this scale (logical units per pixel).
    #[arg(long, value_name = "SCALE")]
    pub pixels: Option<f64>,
}

impl Frame {
    /// Builds a position in this frame.
    pub fn at(&self, x: f64, y: f64) -> anyhow::Result<Position> {
        let output = self.output.map(OutputId);
        Ok(match (&self.window, self.pixels) {
            (Some(w), _) => Position::Window {
                window: parse_window_ref(w)?,
                x,
                y,
            },
            (None, Some(scale)) => Position::Pixels {
                output,
                x,
                y,
                scale,
            },
            (None, None) => Position::Output { output, x, y },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_scenario::WindowRef;

    #[test]
    fn builds_positions() {
        let f = Frame::default();
        assert_eq!(f.at(1.0, 2.0).unwrap(), Position::output(1.0, 2.0));
        let f = Frame {
            window: Some("app:x".into()),
            ..Frame::default()
        };
        assert_eq!(
            f.at(3.0, 4.0).unwrap(),
            Position::window(WindowRef::app_id("x"), 3.0, 4.0)
        );
        let f = Frame {
            pixels: Some(1.5),
            output: Some(2),
            ..Frame::default()
        };
        assert_eq!(
            f.at(10.0, 20.0).unwrap(),
            Position::Pixels {
                output: Some(OutputId(2)),
                x: 10.0,
                y: 20.0,
                scale: 1.5
            }
        );
    }
}
