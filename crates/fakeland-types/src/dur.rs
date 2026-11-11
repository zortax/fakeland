//! Human-readable spans for JSON documents. `Dur` wraps `Nanos` and reads strings with a unit
//! (`"250ms"`, `"2s"`, `"1500us"`, `"16.667ms"`) or a bare number of milliseconds.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Nanos;

/// A span that serializes as a string with a unit and parses strings or bare milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dur(pub Nanos);

/// A string that does not name a span.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid duration {input:?}: {reason}")]
pub struct DurParseError {
    input: String,
    reason: &'static str,
}

impl Dur {
    /// The zero span.
    pub const ZERO: Self = Self(Nanos::ZERO);

    /// Builds a span from nanoseconds.
    pub const fn from_nanos(ns: i64) -> Self {
        Self(Nanos::from_nanos(ns))
    }

    /// Builds a span from microseconds.
    pub const fn from_micros(us: i64) -> Self {
        Self(Nanos::from_micros(us))
    }

    /// Builds a span from milliseconds.
    pub const fn from_millis(ms: i64) -> Self {
        Self(Nanos::from_millis(ms))
    }

    /// Builds a span from seconds.
    pub const fn from_secs(s: i64) -> Self {
        Self(Nanos::from_secs(s))
    }

    /// Builds a span from fractional milliseconds.
    pub fn from_millis_f64(ms: f64) -> Self {
        Self(Nanos::from_millis_f64(ms))
    }

    /// Builds a span from fractional seconds.
    pub fn from_secs_f64(s: f64) -> Self {
        Self(Nanos::from_secs_f64(s))
    }

    /// Returns the span as `Nanos`.
    pub const fn nanos(self) -> Nanos {
        self.0
    }
}

impl From<Nanos> for Dur {
    fn from(n: Nanos) -> Self {
        Self(n)
    }
}

impl From<Dur> for Nanos {
    fn from(d: Dur) -> Self {
        d.0
    }
}

const UNITS: &[(&str, i128)] = &[
    ("ns", 1),
    ("us", 1_000),
    ("µs", 1_000),
    ("ms", 1_000_000),
    ("s", 1_000_000_000),
    ("min", 60_000_000_000),
    ("m", 60_000_000_000),
    ("h", 3_600_000_000_000),
];

impl FromStr for Dur {
    type Err = DurParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let fail = |reason| DurParseError {
            input: input.to_owned(),
            reason,
        };
        let s = input.trim();
        let (negative, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let number_end = s
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(s.len());
        let (number, unit) = s.split_at(number_end);
        let unit = unit.trim();
        let scale = if unit.is_empty() {
            1_000_000
        } else {
            UNITS
                .iter()
                .find(|(name, _)| *name == unit)
                .map(|(_, scale)| *scale)
                .ok_or_else(|| fail("unknown unit"))?
        };
        let (int, frac) = number.split_once('.').unwrap_or((number, ""));
        if int.is_empty() && frac.is_empty() {
            return Err(fail("missing number"));
        }
        if frac.contains('.') {
            return Err(fail("more than one decimal point"));
        }
        let int: i128 = if int.is_empty() {
            0
        } else {
            int.parse().map_err(|_| fail("number out of range"))?
        };
        let mut frac_ns: i128 = 0;
        if !frac.is_empty() {
            let digits = frac.len().min(18) as u32;
            let value: i128 = frac[..digits as usize]
                .parse()
                .map_err(|_| fail("invalid fraction"))?;
            let denom = 10i128.pow(digits);
            frac_ns = (value * scale * 2 + denom) / (denom * 2);
        }
        let total = int
            .checked_mul(scale)
            .and_then(|v| v.checked_add(frac_ns))
            .filter(|v| *v <= i64::MAX as i128)
            .ok_or_else(|| fail("number out of range"))?;
        let ns = total as i64;
        Ok(Self(Nanos(if negative { -ns } else { ns })))
    }
}

impl fmt::Display for Dur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ns = self.0.as_nanos();
        if ns < 0 {
            f.write_str("-")?;
        }
        let abs = ns.unsigned_abs();
        if abs == 0 {
            return f.write_str("0ms");
        }
        if abs.is_multiple_of(1_000_000_000) {
            return write!(f, "{}s", abs / 1_000_000_000);
        }
        let (scale, digits, unit) = if abs >= 1_000_000 {
            (1_000_000, 6, "ms")
        } else if abs >= 1_000 {
            (1_000, 3, "us")
        } else {
            (1, 0, "ns")
        };
        let int = abs / scale;
        let frac = abs % scale;
        if frac == 0 {
            return write!(f, "{int}{unit}");
        }
        let frac = format!("{frac:0digits$}");
        write!(f, "{int}.{}{unit}", frac.trim_end_matches('0'))
    }
}

impl Serialize for Dur {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct DurVisitor;

impl Visitor<'_> for DurVisitor {
    type Value = Dur;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a duration string such as \"250ms\" or a number of milliseconds")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Dur, E> {
        v.parse().map_err(E::custom)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Dur, E> {
        v.checked_mul(1_000_000)
            .map(Dur::from_nanos)
            .ok_or_else(|| E::custom("duration out of range"))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Dur, E> {
        i64::try_from(v)
            .map_err(|_| E::custom("duration out of range"))
            .and_then(|v| self.visit_i64(v))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Dur, E> {
        if !v.is_finite() || v.abs() > i64::MAX as f64 / 1e6 {
            return Err(E::custom("duration out of range"));
        }
        Ok(Dur::from_millis_f64(v))
    }
}

impl<'de> Deserialize<'de> for Dur {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DurVisitor)
    }
}

impl JsonSchema for Dur {
    fn schema_name() -> Cow<'static, str> {
        "Dur".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A duration: a string with a unit (ns, us, ms, s, m, h), for example \"250ms\" or \"16.667ms\", or a number of milliseconds.",
            "anyOf": [
                {
                    "type": "string",
                    "pattern": "^\\s*-?([0-9]+(\\.[0-9]*)?|\\.[0-9]+)\\s*(ns|us|µs|ms|s|min|m|h)?\\s*$"
                },
                { "type": "number" }
            ]
        })
    }
}

/// Serde helpers that store a `Nanos` field as a `Dur`. Use with `#[serde(with = "...")]` and
/// `#[schemars(with = "Dur")]`.
pub mod as_dur {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Dur;
    use crate::Nanos;

    /// Writes the span as a `Dur` string.
    pub fn serialize<S: Serializer>(value: &Nanos, serializer: S) -> Result<S::Ok, S::Error> {
        Dur(*value).serialize(serializer)
    }

    /// Reads a span from a `Dur` string or a number of milliseconds.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Nanos, D::Error> {
        Dur::deserialize(deserializer).map(|d| d.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Dur {
        s.parse().unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn parses_units() {
        assert_eq!(parse("250ms"), Dur::from_millis(250));
        assert_eq!(parse("2s"), Dur::from_secs(2));
        assert_eq!(parse("1500us"), Dur::from_micros(1500));
        assert_eq!(parse("1500µs"), Dur::from_micros(1500));
        assert_eq!(parse("16.667ms"), Dur::from_nanos(16_667_000));
        assert_eq!(parse("1.5 s"), Dur::from_millis(1500));
        assert_eq!(parse("12ns"), Dur::from_nanos(12));
        assert_eq!(parse("2m"), Dur::from_secs(120));
        assert_eq!(parse("1min"), Dur::from_secs(60));
        assert_eq!(parse("1h"), Dur::from_secs(3600));
        assert_eq!(parse("40"), Dur::from_millis(40));
        assert_eq!(parse(".5ms"), Dur::from_micros(500));
        assert_eq!(parse("-3ms"), Dur::from_millis(-3));
        assert_eq!(parse("0.0000005ms"), Dur::from_nanos(1));
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "ms", "1.2.3ms", "5 parsecs", "abc", "--1ms", "1e3ms"] {
            assert!(s.parse::<Dur>().is_err(), "{s}");
        }
    }

    #[test]
    fn formats_with_the_largest_exact_unit() {
        assert_eq!(Dur::from_secs(2).to_string(), "2s");
        assert_eq!(Dur::from_millis(250).to_string(), "250ms");
        assert_eq!(Dur::from_micros(1500).to_string(), "1.5ms");
        assert_eq!(Dur::from_nanos(16_666_667).to_string(), "16.666667ms");
        assert_eq!(Dur::from_micros(500).to_string(), "500us");
        assert_eq!(Dur::from_nanos(1_500).to_string(), "1.5us");
        assert_eq!(Dur::from_nanos(7).to_string(), "7ns");
        assert_eq!(Dur::ZERO.to_string(), "0ms");
        assert_eq!(Dur::from_millis(-4).to_string(), "-4ms");
    }

    #[test]
    fn display_round_trips() {
        for ns in [
            0,
            1,
            999,
            1_000,
            1_001,
            999_999,
            1_000_000,
            16_666_667,
            2_500_000_000,
            -42,
        ] {
            let d = Dur::from_nanos(ns);
            assert_eq!(parse(&d.to_string()), d);
        }
    }

    #[test]
    fn serde_accepts_strings_and_numbers() {
        let d: Dur = serde_json::from_str("\"16.667ms\"").unwrap_or_default();
        assert_eq!(d, Dur::from_nanos(16_667_000));
        let d: Dur = serde_json::from_str("250").unwrap_or_default();
        assert_eq!(d, Dur::from_millis(250));
        let d: Dur = serde_json::from_str("2.5").unwrap_or_default();
        assert_eq!(d, Dur::from_micros(2500));
        assert_eq!(
            serde_json::to_string(&Dur::from_secs(2)).unwrap_or_default(),
            "\"2s\""
        );
        assert!(serde_json::from_str::<Dur>("\"5 lightyears\"").is_err());
    }
}
