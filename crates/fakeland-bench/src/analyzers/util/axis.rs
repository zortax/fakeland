//! The detail string of an injected axis event: source, deltas, `value120` steps and stop.

use serde::{Deserialize, Serialize};

/// The parts of an axis event detail such as `wheel dx=0 dy=15 v120=(0,120) stop`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AxisDetail {
    /// The source: `wheel`, `finger` or `continuous`.
    pub source: String,
    /// The horizontal delta in logical pixels; positive scrolls right.
    pub dx: f64,
    /// The vertical delta in logical pixels; positive scrolls down.
    pub dy: f64,
    /// The `value120` steps.
    pub value120: Option<(i32, i32)>,
    /// The event ends the scroll sequence.
    pub stop: bool,
}

/// Parses an axis detail. Returns `None` when a delta is missing.
pub fn parse_axis_detail(detail: &str) -> Option<AxisDetail> {
    let mut out = AxisDetail::default();
    let mut dx = None;
    let mut dy = None;
    for (i, word) in detail.split_whitespace().enumerate() {
        if let Some(v) = word.strip_prefix("dx=") {
            dx = v.parse().ok();
        } else if let Some(v) = word.strip_prefix("dy=") {
            dy = v.parse().ok();
        } else if let Some(v) = word.strip_prefix("v120=") {
            let v = v.trim_start_matches('(').trim_end_matches(')');
            let (x, y) = v.split_once(',')?;
            out.value120 = Some((x.parse().ok()?, y.parse().ok()?));
        } else if word == "stop" {
            out.stop = true;
        } else if i == 0 {
            out.source = word.to_string();
        }
    }
    out.dx = dx?;
    out.dy = dy?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_part() {
        let d = parse_axis_detail("wheel dx=0 dy=-2.5 v120=(0,-20) stop").unwrap();
        assert_eq!(d.source, "wheel");
        assert_eq!((d.dx, d.dy), (0.0, -2.5));
        assert_eq!(d.value120, Some((0, -20)));
        assert!(d.stop);
        let f = parse_axis_detail("finger dx=1 dy=7.25").unwrap();
        assert_eq!(
            (f.source.as_str(), f.dy, f.value120, f.stop),
            ("finger", 7.25, None, false)
        );
        assert!(parse_axis_detail("button 272 pressed").is_none());
    }
}
