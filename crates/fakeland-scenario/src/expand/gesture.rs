//! Expansion of touchpad gestures: begin, eased updates and end.

use fakeland_types::Nanos;

use super::pointer::motion;
use super::ticks::{check, ticks};
use super::{ExpandError, InputProgram, Primitive};
use crate::Gesture;

fn fingers(gesture: &'static str, min: u32, fingers: u32) -> Result<(), ExpandError> {
    if fingers < min {
        return Err(ExpandError::Fingers {
            gesture,
            min,
            fingers,
        });
    }
    Ok(())
}

pub(super) fn gesture(g: &Gesture) -> Result<InputProgram, ExpandError> {
    let mut p = InputProgram::new();
    match g {
        Gesture::Pinch {
            center,
            scale_from,
            scale_to,
            rotation,
            duration,
            fingers: n,
            easing,
            rate_hz,
        } => {
            fingers("pinch", 2, *n)?;
            for s in [*scale_from, *scale_to] {
                if !(s.is_finite() && s > 0.0) {
                    return Err(ExpandError::Scale(s));
                }
            }
            if !rotation.is_finite() {
                return Err(ExpandError::NotFinite("pinch rotation"));
            }
            easing.validate()?;
            motion(&mut p, Nanos::ZERO, center.clone());
            p.push(Nanos::ZERO, Primitive::PinchBegin { fingers: *n });
            let mut prev = 0.0;
            for (at, f) in ticks(*duration, *rate_hz)?.into_iter().skip(1) {
                let e = easing.apply(f);
                p.push(
                    at,
                    Primitive::PinchUpdate {
                        dx: 0.0,
                        dy: 0.0,
                        scale: scale_from + (scale_to - scale_from) * e,
                        rotation: rotation * (e - prev),
                    },
                );
                prev = e;
            }
            p.push(duration.0, Primitive::PinchEnd { cancelled: false });
        }
        Gesture::Swipe {
            at,
            dx,
            dy,
            duration,
            fingers: n,
            easing,
            rate_hz,
        } => {
            fingers("swipe", 1, *n)?;
            if !dx.is_finite() || !dy.is_finite() {
                return Err(ExpandError::NotFinite("swipe distance"));
            }
            easing.validate()?;
            if let Some(pos) = at {
                motion(&mut p, Nanos::ZERO, pos.clone());
            }
            p.push(Nanos::ZERO, Primitive::SwipeBegin { fingers: *n });
            let mut prev = 0.0;
            for (t, f) in ticks(*duration, *rate_hz)?.into_iter().skip(1) {
                let e = easing.apply(f);
                p.push(
                    t,
                    Primitive::SwipeUpdate {
                        dx: dx * (e - prev),
                        dy: dy * (e - prev),
                    },
                );
                prev = e;
            }
            p.push(duration.0, Primitive::SwipeEnd { cancelled: false });
        }
        Gesture::Hold {
            at,
            duration,
            fingers: n,
            cancelled,
        } => {
            fingers("hold", 1, *n)?;
            check(*duration, 1.0, "hold duration")?;
            if let Some(pos) = at {
                motion(&mut p, Nanos::ZERO, pos.clone());
            }
            p.push(Nanos::ZERO, Primitive::HoldBegin { fingers: *n });
            p.push(
                duration.0,
                Primitive::HoldEnd {
                    cancelled: *cancelled,
                },
            );
        }
    }
    Ok(p)
}
