//! Parsers for command-line values: sizes `WxH`, rectangles `X,Y,W,H`, durations with a unit
//! (milliseconds without one) and durations in seconds.

use fakeland_types::{Dur, Logical, Nanos, Rect, Size};

/// Parses a positive logical size `WxH`.
pub fn parse_size(s: &str) -> Result<Size<Logical>, String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("invalid size {s:?}: use WxH"))?;
    let num = |v: &str| {
        v.trim()
            .parse::<i32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("invalid size {s:?}: use positive integers"))
    };
    Ok(Size::new(num(w)?, num(h)?))
}

/// Parses a logical rectangle `X,Y,W,H` with a positive size.
pub fn parse_rect(s: &str) -> Result<Rect<Logical>, String> {
    let parts: Vec<_> = s.split(',').map(str::trim).collect();
    let bad = || format!("invalid rectangle {s:?}: use X,Y,W,H");
    let [x, y, w, h] = parts.as_slice() else {
        return Err(bad());
    };
    let num = |v: &str| v.parse::<i32>().map_err(|_| bad());
    let rect = Rect::new(num(x)?, num(y)?, num(w)?, num(h)?);
    if rect.is_empty() {
        return Err(format!(
            "invalid rectangle {s:?}: width and height must be positive"
        ));
    }
    Ok(rect)
}

/// Parses a non-negative duration. A bare number is milliseconds; units are ns, us, ms, s, m
/// and h.
pub fn parse_millis(s: &str) -> Result<Nanos, String> {
    let d: Dur = s.parse().map_err(|e| format!("{e}"))?;
    let n = Nanos::from(d);
    if n.is_negative() {
        return Err(format!("invalid duration {s:?}: must not be negative"));
    }
    Ok(n)
}

/// Parses a positive duration. A bare number is seconds; a unit works as in `parse_millis`.
pub fn parse_secs(s: &str) -> Result<Nanos, String> {
    let n = match s.trim().parse::<f64>() {
        Ok(secs) if secs.is_finite() => Nanos::from_secs_f64(secs),
        Ok(_) => return Err(format!("invalid duration {s:?}")),
        Err(_) => parse_millis(s)?,
    };
    if n.0 <= 0 {
        return Err(format!("invalid duration {s:?}: must be positive"));
    }
    Ok(n)
}

/// Parses a positive finite number.
pub fn parse_positive(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| format!("invalid value {s:?}: use a positive number"))
}

/// Parses a finite number.
pub fn parse_finite(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid value {s:?}: use a finite number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("640x480").unwrap(), Size::new(640, 480));
        assert_eq!(parse_size("10X20").unwrap(), Size::new(10, 20));
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("640").is_err());
        assert!(parse_size("ax1").is_err());
    }

    #[test]
    fn rects() {
        assert_eq!(parse_rect("1, 2,3,4").unwrap(), Rect::new(1, 2, 3, 4));
        assert_eq!(parse_rect("-5,0,3,4").unwrap(), Rect::new(-5, 0, 3, 4));
        assert!(parse_rect("1,2,0,4").is_err());
        assert!(parse_rect("1,2,3").is_err());
    }

    #[test]
    fn durations() {
        assert_eq!(parse_millis("7").unwrap(), Nanos::from_millis(7));
        assert_eq!(parse_millis("0.5").unwrap(), Nanos::from_micros(500));
        assert_eq!(parse_millis("250us").unwrap(), Nanos::from_micros(250));
        assert_eq!(parse_millis("2s").unwrap(), Nanos::from_secs(2));
        assert!(parse_millis("-1").is_err());
        assert!(parse_millis("abc").is_err());
        assert_eq!(parse_secs("2").unwrap(), Nanos::from_secs(2));
        assert_eq!(parse_secs("1.5").unwrap(), Nanos::from_millis(1500));
        assert_eq!(parse_secs("300ms").unwrap(), Nanos::from_millis(300));
        assert!(parse_secs("0").is_err());
    }

    #[test]
    fn numbers() {
        assert_eq!(parse_positive("59.94").unwrap(), 59.94);
        assert!(parse_positive("0").is_err());
        assert!(parse_positive("inf").is_err());
        assert_eq!(parse_finite("-2").unwrap(), -2.0);
    }
}
