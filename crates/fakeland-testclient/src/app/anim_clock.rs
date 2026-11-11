//! The animation time of a frame: the render begin time, the frame callback time, or the
//! predicted next presentation time from the last presentation feedback.

use fakeland_types::{Nanos, Timestamp};
use serde::Serialize;

/// Which time drives the animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum AnimClock {
    /// The time when rendering of the frame begins.
    #[default]
    Render,
    /// The time of the last frame callback, in its millisecond resolution.
    Callback,
    /// The first presentation time after render begin on the grid of the last feedback.
    Predicted,
}

/// The time inputs of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimInputs {
    /// The callback_data of the last frame callback.
    pub callback_ms: Option<u32>,
    /// The last presentation time.
    pub presented: Option<Timestamp>,
    /// The refresh period of the last presentation.
    pub refresh: Option<Nanos>,
}

impl AnimClock {
    /// Returns the animation time of a frame that begins rendering at `now`. Each clock falls
    /// back to `now` until its input exists.
    pub fn time(self, now: Timestamp, inputs: &AnimInputs) -> Timestamp {
        match self {
            Self::Render => now,
            Self::Callback => inputs.callback_ms.map_or(now, |ms| expand_millis(now, ms)),
            Self::Predicted => match (inputs.presented, inputs.refresh) {
                (Some(last), Some(refresh)) if refresh.0 > 0 => next_on_grid(now, last, refresh),
                _ => now,
            },
        }
    }
}

/// Expands a 32-bit millisecond time to the full timestamp nearest to `now`.
pub fn expand_millis(now: Timestamp, ms: u32) -> Timestamp {
    let now_ms = now.0 / 1_000_000;
    let base = now_ms & !0xffff_ffff;
    let candidates = [
        base.wrapping_sub(1 << 32) | ms as u64,
        base | ms as u64,
        (base + (1 << 32)) | ms as u64,
    ];
    let best = candidates
        .into_iter()
        .min_by_key(|c| c.abs_diff(now_ms))
        .unwrap_or(ms as u64);
    Timestamp(best * 1_000_000)
}

/// Returns the first grid point `last + k * refresh` strictly after `now`.
pub fn next_on_grid(now: Timestamp, last: Timestamp, refresh: Nanos) -> Timestamp {
    if now < last {
        return last;
    }
    let k = now.since(last).0 / refresh.0 + 1;
    last + Nanos(k * refresh.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_clock_is_now() {
        let now = Timestamp(123);
        assert_eq!(AnimClock::Render.time(now, &AnimInputs::default()), now);
        assert_eq!(AnimClock::Callback.time(now, &AnimInputs::default()), now);
        assert_eq!(AnimClock::Predicted.time(now, &AnimInputs::default()), now);
    }

    #[test]
    fn callback_millis_expand_near_now() {
        let now = Timestamp(5_000_000_123);
        assert_eq!(expand_millis(now, 4999), Timestamp(4_999_000_000));
        // Past the 32-bit wrap: now is 2^32 + 5 ms, the callback said 2^32 - 3 ms.
        let now = Timestamp(((1u64 << 32) + 5) * 1_000_000);
        let t = expand_millis(now, u32::MAX - 2);
        assert_eq!(t, Timestamp(((1u64 << 32) - 3) * 1_000_000));
        let inputs = AnimInputs {
            callback_ms: Some(4999),
            ..AnimInputs::default()
        };
        assert_eq!(
            AnimClock::Callback.time(Timestamp(5_000_000_000), &inputs),
            Timestamp(4_999_000_000)
        );
    }

    #[test]
    fn prediction_uses_the_feedback_grid() {
        let refresh = Nanos(16_666_667);
        let last = Timestamp(1_000_000_000);
        assert_eq!(next_on_grid(last, last, refresh), last + refresh);
        let now = last + Nanos(20_000_000);
        assert_eq!(
            next_on_grid(now, last, refresh),
            last + Nanos(2 * 16_666_667)
        );
        assert_eq!(next_on_grid(Timestamp(5), last, refresh), last);
        let inputs = AnimInputs {
            presented: Some(last),
            refresh: Some(refresh),
            callback_ms: None,
        };
        assert_eq!(
            AnimClock::Predicted.time(now, &inputs),
            last + Nanos(33_333_334)
        );
    }
}
