//! Expansion of pointer actions: motion, buttons, clicks, single axis events, glides and drags.

use fakeland_types::{Dur, Nanos};

use super::ticks::ticks;
use super::{ExpandError, InputProgram, Primitive};
use crate::{Button, Drag, PointerGlide, Position, ScrollSource};

pub(super) fn motion(p: &mut InputProgram, at: Nanos, pos: Position) {
    p.push(at, Primitive::PointerMotion { pos });
    p.push(at, Primitive::PointerFrame);
}

pub(super) fn button(p: &mut InputProgram, at: Nanos, button: Button, pressed: bool) {
    p.push(at, Primitive::PointerButton { button, pressed });
    p.push(at, Primitive::PointerFrame);
}

pub(super) fn click(
    at: Option<&Position>,
    which: Button,
    count: u32,
    interval: Dur,
    hold: Dur,
) -> Result<InputProgram, ExpandError> {
    if count == 0 {
        return Err(ExpandError::Count("click count"));
    }
    if interval.0.is_negative() || hold.0.is_negative() {
        return Err(ExpandError::NegativeDuration("click interval and hold"));
    }
    let mut p = InputProgram::new();
    if let Some(pos) = at {
        motion(&mut p, Nanos::ZERO, pos.clone());
    }
    let hold = if count > 1 {
        hold.0.min(interval.0 / 2)
    } else {
        hold.0
    };
    for i in 0..i64::from(count) {
        let press = interval.0 * i;
        button(&mut p, press, which, true);
        button(&mut p, press + hold, which, false);
    }
    Ok(p)
}

pub(super) fn axis(
    dx: f64,
    dy: f64,
    source: ScrollSource,
    value120: Option<(i32, i32)>,
) -> Result<InputProgram, ExpandError> {
    if !dx.is_finite() || !dy.is_finite() {
        return Err(ExpandError::NotFinite("axis distance"));
    }
    let value120 = match (value120, source) {
        (Some(v), _) => Some(v),
        (None, ScrollSource::Wheel { detent, .. }) => {
            if !(detent.is_finite() && detent > 0.0) {
                return Err(ExpandError::Detent(detent));
            }
            let steps = |d: f64| (d / detent * 120.0).round() as i32;
            Some((steps(dx), steps(dy)))
        }
        (None, _) => None,
    };
    let mut p = InputProgram::new();
    p.push(
        Nanos::ZERO,
        Primitive::PointerAxis {
            dx,
            dy,
            source,
            value120,
            stop: false,
        },
    );
    p.push(Nanos::ZERO, Primitive::PointerFrame);
    Ok(p)
}

/// Appends glide motions shifted by `start`.
fn glide_into(p: &mut InputProgram, start: Nanos, glide: &PointerGlide) -> Result<(), ExpandError> {
    glide.easing.validate()?;
    let sampler = glide.path.sampler()?;
    for (offset, progress) in ticks(glide.duration, glide.rate_hz)? {
        let pos = sampler.point_at(glide.easing.apply(progress));
        motion(p, start + offset, pos);
    }
    Ok(())
}

pub(super) fn glide(glide: &PointerGlide) -> Result<InputProgram, ExpandError> {
    let mut p = InputProgram::new();
    glide_into(&mut p, Nanos::ZERO, glide)?;
    Ok(p)
}

pub(super) fn drag(drag: &Drag) -> Result<InputProgram, ExpandError> {
    if drag.hold.0.is_negative() {
        return Err(ExpandError::NegativeDuration("drag hold"));
    }
    if !drag.from.is_finite() {
        return Err(ExpandError::NotFinite("drag start"));
    }
    let mut p = InputProgram::new();
    motion(&mut p, Nanos::ZERO, drag.from.clone());
    button(&mut p, Nanos::ZERO, drag.button, true);
    let glide = PointerGlide {
        path: drag.path.clone(),
        duration: drag.duration,
        easing: drag.easing,
        rate_hz: drag.rate_hz,
    };
    glide_into(&mut p, drag.hold.0, &glide)?;
    let release = drag.hold.0 + drag.duration.0 + drag.hold.0;
    button(&mut p, release, drag.button, false);
    Ok(p)
}
