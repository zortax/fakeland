//! Expansion of typed text: one press and release per character at the typing cadence, or one
//! text-input commit.

use fakeland_types::{Nanos, SplitMix64};

use super::keyboard::key;
use super::{ExpandError, InputProgram, Primitive};
use crate::{Key, TextVia, TypingCadence};

/// Returns the press offsets of `n` keys. Press `i` lies at `i * interval` moved by a seeded
/// offset of at most `jitter * interval / 2`; the first press lies at zero.
pub(super) fn press_offsets(n: usize, cadence: &TypingCadence) -> Result<Vec<Nanos>, ExpandError> {
    if !cadence.wpm.is_finite() || cadence.wpm <= 0.0 {
        return Err(ExpandError::Wpm(cadence.wpm));
    }
    if !(0.0..1.0).contains(&cadence.jitter) {
        return Err(ExpandError::Jitter(cadence.jitter));
    }
    if cadence.hold.0.is_negative() {
        return Err(ExpandError::NegativeDuration("key hold"));
    }
    let interval = cadence.interval_s() * 1e9;
    let mut rng = SplitMix64::new(cadence.seed);
    Ok((0..n)
        .map(|i| {
            let shift = if i == 0 {
                0.0
            } else {
                rng.next_signed_f64() * cadence.jitter * interval / 2.0
            };
            Nanos((i as f64 * interval + shift).round() as i64)
        })
        .collect())
}

pub(super) fn type_text(
    text: &str,
    cadence: &TypingCadence,
    via: TextVia,
) -> Result<InputProgram, ExpandError> {
    let mut p = InputProgram::new();
    if via == TextVia::TextInput {
        p.push(
            Nanos::ZERO,
            Primitive::Text {
                text: text.to_owned(),
            },
        );
        return Ok(p);
    }
    let chars: Vec<char> = text.chars().collect();
    let presses = press_offsets(chars.len(), cadence)?;
    for (i, c) in chars.iter().enumerate() {
        let k = Key::char(*c);
        let at = presses[i];
        let hold = match presses.get(i + 1) {
            Some(next) => cadence.hold.0.min((*next - at) / 2),
            None => cadence.hold.0,
        };
        key(&mut p, at, k.clone(), true);
        key(&mut p, at + hold, k, false);
    }
    p.sort();
    Ok(p)
}
