//! Expansion of touch actions.

use fakeland_types::{Dur, Nanos};

use super::ticks::ticks;
use super::{ExpandError, InputProgram, Primitive};
use crate::{Easing, Path, Position};

fn framed(p: &mut InputProgram, at: Nanos, primitive: Primitive) {
    p.push(at, primitive);
    p.push(at, Primitive::TouchFrame);
}

pub(super) fn single(primitive: Primitive) -> InputProgram {
    let mut p = InputProgram::new();
    framed(&mut p, Nanos::ZERO, primitive);
    p
}

pub(super) fn tap(id: u32, pos: &Position, hold: Dur) -> Result<InputProgram, ExpandError> {
    if hold.0.is_negative() {
        return Err(ExpandError::NegativeDuration("tap hold"));
    }
    let mut p = single(Primitive::TouchDown {
        id,
        pos: pos.clone(),
    });
    framed(&mut p, hold.0, Primitive::TouchUp { id });
    Ok(p)
}

pub(super) fn glide(
    id: u32,
    path: &Path,
    duration: Dur,
    easing: Easing,
    rate_hz: f64,
) -> Result<InputProgram, ExpandError> {
    easing.validate()?;
    let sampler = path.sampler()?;
    let mut p = InputProgram::new();
    let ticks = ticks(duration, rate_hz)?;
    for (i, (at, progress)) in ticks.iter().enumerate() {
        let pos = sampler.point_at(easing.apply(*progress));
        let primitive = if i == 0 {
            Primitive::TouchDown { id, pos }
        } else {
            Primitive::TouchMotion { id, pos }
        };
        framed(&mut p, *at, primitive);
    }
    framed(&mut p, duration.0, Primitive::TouchUp { id });
    Ok(p)
}
