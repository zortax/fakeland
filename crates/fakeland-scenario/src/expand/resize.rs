//! Expansion of window resizes and resize storms.

use fakeland_types::{Dur, Logical, Nanos, Size};

use super::ticks::ticks;
use super::{ExpandError, InputProgram, Primitive};
use crate::{Easing, WindowRef};

fn check_size(size: Size<Logical>) -> Result<(), ExpandError> {
    if size.w <= 0 || size.h <= 0 {
        return Err(ExpandError::Size {
            w: size.w,
            h: size.h,
        });
    }
    Ok(())
}

pub(super) fn resize(window: &WindowRef, size: Size<Logical>) -> Result<InputProgram, ExpandError> {
    check_size(size)?;
    let mut p = InputProgram::new();
    p.push(
        Nanos::ZERO,
        Primitive::WindowResize {
            window: window.clone(),
            size,
        },
    );
    Ok(p)
}

/// Configures eased sizes from `from` to `to` at `rate_hz`, without repeating a size.
pub(super) fn storm(
    window: &WindowRef,
    from: Size<Logical>,
    to: Size<Logical>,
    duration: Dur,
    rate_hz: f64,
    easing: Easing,
) -> Result<InputProgram, ExpandError> {
    check_size(from)?;
    check_size(to)?;
    easing.validate()?;
    let mut p = InputProgram::new();
    let mut last: Option<Size<Logical>> = None;
    for (at, f) in ticks(duration, rate_hz)? {
        let e = easing.apply(f);
        let lerp = |a: i32, b: i32| (a as f64 + (b - a) as f64 * e).round() as i32;
        let size = Size::new(lerp(from.w, to.w), lerp(from.h, to.h));
        if last != Some(size) {
            last = Some(size);
            p.push(
                at,
                Primitive::WindowResize {
                    window: window.clone(),
                    size,
                },
            );
        }
    }
    Ok(p)
}
