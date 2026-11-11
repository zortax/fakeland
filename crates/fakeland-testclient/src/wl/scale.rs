//! Surface scale: the strategy the client uses (fractional through wp_fractional_scale and
//! wp_viewport, integer through wl_surface.set_buffer_scale, or none) and the buffer size that a
//! logical size and a scale give.

use fakeland_types::{Logical, Physical, Size};
use serde::Serialize;

/// The scale strategy that the command line asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMode {
    /// Fractional when the compositor offers wp_fractional_scale and wp_viewporter, else
    /// integer.
    #[default]
    Auto,
    /// Fractional; the client fails without the two globals.
    Fractional,
    /// Integer buffer scale from wl_surface.preferred_buffer_scale or the entered outputs.
    Integer,
    /// Always scale 1.
    None,
}

/// The scale of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// A fractional scale in 120ths with a viewport destination.
    Fractional(u32),
    /// An integer buffer scale.
    Integer(i32),
}

impl Scale {
    /// Returns buffer pixels per logical unit.
    pub fn factor(self) -> f64 {
        match self {
            Self::Fractional(s) => s as f64 / 120.0,
            Self::Integer(n) => n as f64,
        }
    }

    /// Returns the wl_surface buffer scale.
    pub fn buffer_scale(self) -> i32 {
        match self {
            Self::Fractional(_) => 1,
            Self::Integer(n) => n,
        }
    }

    /// Returns the buffer size for a logical size. A fractional size rounds half up, as
    /// wp_fractional_scale_v1 asks.
    pub fn buffer_size(self, logical: Size<Logical>) -> Size<Physical> {
        match self {
            Self::Fractional(s) => {
                let px = |v: i32| ((v as i64 * s as i64 + 60) / 120).max(1) as i32;
                Size::new(px(logical.w), px(logical.h))
            }
            Self::Integer(n) => Size::new(logical.w * n, logical.h * n),
        }
    }

    /// Returns the name of the strategy for the log.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Fractional(_) => "fractional",
            Self::Integer(1) => "none",
            Self::Integer(_) => "integer",
        }
    }

    /// Returns the scale in 120ths.
    pub fn scale120(self) -> u32 {
        match self {
            Self::Fractional(s) => s,
            Self::Integer(n) => n.max(1) as u32 * 120,
        }
    }
}

/// The strategy in use after the globals are known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// Fractional scaling.
    Fractional,
    /// Integer buffer scale.
    Integer,
    /// Scale 1.
    Unscaled,
}

/// The scale hints the compositor sent.
#[derive(Clone, Debug)]
pub struct ScaleState {
    strategy: Strategy,
    fractional: u32,
    preferred: Option<i32>,
    outputs: i32,
}

impl ScaleState {
    /// Builds the state for a strategy. Scale 1 holds until a hint arrives.
    pub fn new(strategy: Strategy) -> Self {
        Self {
            strategy,
            fractional: 120,
            preferred: None,
            outputs: 1,
        }
    }

    /// Picks the strategy for a mode and the available globals. Returns `None` when the mode
    /// needs globals that are missing.
    pub fn pick(mode: ScaleMode, has_fractional: bool) -> Option<Strategy> {
        match mode {
            ScaleMode::Auto if has_fractional => Some(Strategy::Fractional),
            ScaleMode::Auto | ScaleMode::Integer => Some(Strategy::Integer),
            ScaleMode::Fractional => has_fractional.then_some(Strategy::Fractional),
            ScaleMode::None => Some(Strategy::Unscaled),
        }
    }

    /// Records wp_fractional_scale_v1.preferred_scale.
    pub fn set_fractional(&mut self, scale120: u32) {
        self.fractional = scale120.max(1);
    }

    /// Records wl_surface.preferred_buffer_scale.
    pub fn set_preferred(&mut self, scale: i32) {
        self.preferred = Some(scale.max(1));
    }

    /// Records the largest scale of the outputs the surface is on.
    pub fn set_outputs(&mut self, scale: i32) {
        self.outputs = scale.max(1);
    }

    /// Returns the current scale.
    pub fn current(&self) -> Scale {
        match self.strategy {
            Strategy::Fractional => Scale::Fractional(self.fractional),
            Strategy::Integer => Scale::Integer(self.preferred.unwrap_or(self.outputs)),
            Strategy::Unscaled => Scale::Integer(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_buffer_size_rounds_half_up() {
        let s = Scale::Fractional(180);
        assert_eq!(s.buffer_size(Size::new(640, 480)), Size::new(960, 720));
        assert_eq!(s.buffer_size(Size::new(641, 1)), Size::new(962, 2));
        assert_eq!(
            Scale::Fractional(150).buffer_size(Size::new(3, 3)),
            Size::new(4, 4)
        );
        assert_eq!(s.factor(), 1.5);
        assert_eq!(s.buffer_scale(), 1);
    }

    #[test]
    fn integer_buffer_size() {
        let s = Scale::Integer(2);
        assert_eq!(s.buffer_size(Size::new(640, 480)), Size::new(1280, 960));
        assert_eq!(s.buffer_scale(), 2);
        assert_eq!(s.kind(), "integer");
        assert_eq!(Scale::Integer(1).kind(), "none");
        assert_eq!(s.scale120(), 240);
    }

    #[test]
    fn strategy_follows_mode_and_globals() {
        assert_eq!(
            ScaleState::pick(ScaleMode::Auto, true),
            Some(Strategy::Fractional)
        );
        assert_eq!(
            ScaleState::pick(ScaleMode::Auto, false),
            Some(Strategy::Integer)
        );
        assert_eq!(ScaleState::pick(ScaleMode::Fractional, false), None);
        assert_eq!(
            ScaleState::pick(ScaleMode::None, true),
            Some(Strategy::Unscaled)
        );
    }

    #[test]
    fn integer_prefers_the_preferred_buffer_scale() {
        let mut st = ScaleState::new(Strategy::Integer);
        assert_eq!(st.current(), Scale::Integer(1));
        st.set_outputs(3);
        assert_eq!(st.current(), Scale::Integer(3));
        st.set_preferred(2);
        assert_eq!(st.current(), Scale::Integer(2));
        let mut fr = ScaleState::new(Strategy::Fractional);
        fr.set_fractional(150);
        assert_eq!(fr.current(), Scale::Fractional(150));
        let mut un = ScaleState::new(Strategy::Unscaled);
        un.set_preferred(2);
        assert_eq!(un.current(), Scale::Integer(1));
    }
}
