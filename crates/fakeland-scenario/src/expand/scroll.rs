//! Expansion of scroll glides and flings. Each event carries the integral of the velocity since
//! the previous event, so the total distance equals the curve integral. Wheels quantize to
//! `value120` steps and carry the remainder forward.

use fakeland_types::Nanos;

use super::pointer::motion;
use super::ticks::{check, ticks};
use super::{ExpandError, InputProgram, Primitive};
use crate::{Fling, ScrollAxis, ScrollGlide, ScrollSource};

const NON_KINETIC_PAUSE: Nanos = Nanos::from_millis(150);

fn axis_event(
    p: &mut InputProgram,
    at: Nanos,
    axis: ScrollAxis,
    d: f64,
    src: ScrollSource,
    v120: Option<i32>,
) {
    let (dx, dy) = match axis {
        ScrollAxis::Vertical => (0.0, d),
        ScrollAxis::Horizontal => (d, 0.0),
    };
    let value120 = v120.map(|v| match axis {
        ScrollAxis::Vertical => (0, v),
        ScrollAxis::Horizontal => (v, 0),
    });
    p.push(
        at,
        Primitive::PointerAxis {
            dx,
            dy,
            source: src,
            value120,
            stop: false,
        },
    );
    p.push(at, Primitive::PointerFrame);
}

fn stop(p: &mut InputProgram, at: Nanos, src: ScrollSource) {
    p.push(
        at,
        Primitive::PointerAxis {
            dx: 0.0,
            dy: 0.0,
            source: src,
            value120: None,
            stop: true,
        },
    );
    p.push(at, Primitive::PointerFrame);
}

/// Emits events for cumulative displacements `(offset, total_px)`.
fn emit(
    p: &mut InputProgram,
    axis: ScrollAxis,
    source: ScrollSource,
    samples: &[(Nanos, f64)],
    period: Nanos,
) -> Result<(), ExpandError> {
    match source {
        ScrollSource::Wheel { hires, detent } => {
            if !(detent.is_finite() && detent > 0.0) {
                return Err(ExpandError::Detent(detent));
            }
            let mut sent = 0i64;
            for &(at, total) in samples {
                let detents = total / detent;
                let target = if hires {
                    (detents * 120.0).round() as i64
                } else {
                    detents.round() as i64 * 120
                };
                let step = target - sent;
                if step != 0 {
                    sent = target;
                    let d = step as f64 / 120.0 * detent;
                    axis_event(p, at, axis, d, source, Some(step as i32));
                }
            }
        }
        ScrollSource::Finger { kinetic } => {
            let mut prev = 0.0;
            let mut last = Nanos::ZERO;
            for &(at, total) in samples {
                let d = total - prev;
                prev = total;
                if d != 0.0 {
                    axis_event(p, at, axis, d, source, None);
                    last = at;
                }
            }
            let pause = if kinetic { period } else { NON_KINETIC_PAUSE };
            stop(p, last + pause, source);
        }
        ScrollSource::Continuous => {
            let mut prev = 0.0;
            for &(at, total) in samples {
                let d = total - prev;
                prev = total;
                if d != 0.0 {
                    axis_event(p, at, axis, d, source, None);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn scroll_glide(g: &ScrollGlide) -> Result<InputProgram, ExpandError> {
    g.velocity.validate()?;
    let mut p = InputProgram::new();
    if let Some(at) = &g.at {
        motion(&mut p, Nanos::ZERO, at.clone());
    }
    let duration_s = g.duration.0.as_secs_f64();
    let samples: Vec<(Nanos, f64)> = ticks(g.duration, g.rate_hz)?
        .into_iter()
        .skip(1)
        .map(|(at, f)| (at, g.velocity.displacement_at(f, duration_s)))
        .collect();
    let period = Nanos::from_secs_f64(1.0 / g.rate_hz);
    emit(&mut p, g.axis, g.source, &samples, period)?;
    Ok(p)
}

pub(super) fn fling(f: &Fling) -> Result<InputProgram, ExpandError> {
    if !f.velocity.is_finite() {
        return Err(ExpandError::NotFinite("fling velocity"));
    }
    if f.time_constant.0.as_nanos() <= 0 {
        return Err(ExpandError::NegativeDuration("fling time constant"));
    }
    check(f.duration, f.rate_hz, "fling duration")?;
    let mut p = InputProgram::new();
    if let Some(at) = &f.at {
        motion(&mut p, Nanos::ZERO, at.clone());
    }
    let samples: Vec<(Nanos, f64)> = ticks(f.duration, f.rate_hz)?
        .into_iter()
        .skip(1)
        .map(|(at, _)| (at, f.displacement_at(at.as_secs_f64())))
        .collect();
    let period = Nanos::from_secs_f64(1.0 / f.rate_hz);
    emit(&mut p, f.axis, ScrollSource::finger(true), &samples, period)?;
    Ok(p)
}
