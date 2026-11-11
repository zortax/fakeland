//! Typing cadence: words per minute, seeded jitter and key hold time.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How text reaches the client.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextVia {
    /// One key press and release per character, at the typing cadence.
    #[default]
    Keys,
    /// One text-input commit with the whole text.
    TextInput,
}

/// The timing of typed text. A word is five characters, so the interval between key presses is
/// `60 s / (5 * wpm)`. Jitter moves each press by a seeded uniform fraction of the interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TypingCadence {
    /// Words per minute.
    pub wpm: f64,
    /// The jitter as a fraction of the interval, in `0..1`.
    pub jitter: f64,
    /// The time each key stays down. It stays below the gap to the next press.
    pub hold: Dur,
    /// The jitter seed.
    pub seed: u64,
}

impl Default for TypingCadence {
    fn default() -> Self {
        Self {
            wpm: 300.0,
            jitter: 0.0,
            hold: Dur::from_millis(20),
            seed: 0,
        }
    }
}

impl TypingCadence {
    /// Builds a cadence at a typing speed.
    pub fn wpm(wpm: f64) -> Self {
        Self {
            wpm,
            ..Self::default()
        }
    }

    /// Sets the jitter fraction and its seed.
    pub fn with_jitter(mut self, jitter: f64, seed: u64) -> Self {
        self.jitter = jitter;
        self.seed = seed;
        self
    }

    /// Sets the key hold time.
    pub fn with_hold(mut self, hold: Dur) -> Self {
        self.hold = hold;
        self
    }

    /// Returns the nominal interval between key presses in seconds.
    pub fn interval_s(&self) -> f64 {
        60.0 / (5.0 * self.wpm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_from_wpm() {
        assert!((TypingCadence::wpm(60.0).interval_s() - 0.2).abs() < 1e-12);
        assert!((TypingCadence::default().interval_s() - 0.04).abs() < 1e-12);
    }
}
