//! Client-side phases on the wire. A span is a begin and an end timestamp, and it must not end
//! before it begins.

use fakeland_types::{Nanos, Timestamp};

use super::time::{TimeError, WireTime};

/// The begin and end of a span in their wire layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WireSpan {
    /// The begin time.
    pub begin: WireTime,
    /// The end time.
    pub end: WireTime,
}

/// A wire span that the protocol rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SpanError {
    /// The begin time is invalid.
    #[error("invalid begin time: {0}")]
    Begin(TimeError),
    /// The end time is invalid.
    #[error("invalid end time: {0}")]
    End(TimeError),
    /// The span ends before it begins.
    #[error("the span ends {0} before it begins")]
    EndsBeforeBegin(Nanos),
}

impl WireSpan {
    /// Encodes a span. The encoding keeps the order of the two times, so the receiver rejects a
    /// span that ends before it begins.
    pub const fn new(begin: Timestamp, end: Timestamp) -> Self {
        Self {
            begin: WireTime::from_timestamp(begin),
            end: WireTime::from_timestamp(end),
        }
    }

    /// Decodes and checks the span. Returns the begin and end timestamps.
    pub fn decode(self) -> Result<(Timestamp, Timestamp), SpanError> {
        let begin = self.begin.to_timestamp().map_err(SpanError::Begin)?;
        let end = self.end.to_timestamp().map_err(SpanError::End)?;
        if end < begin {
            return Err(SpanError::EndsBeforeBegin(begin - end));
        }
        Ok((begin, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ordered_spans() {
        let (b, e) = (Timestamp(1_500_000_000), Timestamp(2_250_000_001));
        assert_eq!(WireSpan::new(b, e).decode(), Ok((b, e)));
        assert_eq!(WireSpan::new(b, b).decode(), Ok((b, b)));
    }

    #[test]
    fn rejects_reversed_spans() {
        let span = WireSpan::new(Timestamp(2_000), Timestamp(1_500));
        assert_eq!(span.decode(), Err(SpanError::EndsBeforeBegin(Nanos(500))));
    }

    #[test]
    fn reports_which_time_is_invalid() {
        let bad = WireTime {
            sec_hi: 0,
            sec_lo: 0,
            nsec: 2_000_000_000,
        };
        let good = WireTime::default();
        let begin_bad = WireSpan {
            begin: bad,
            end: good,
        };
        let end_bad = WireSpan {
            begin: good,
            end: bad,
        };
        assert!(matches!(begin_bad.decode(), Err(SpanError::Begin(_))));
        assert!(matches!(end_bad.decode(), Err(SpanError::End(_))));
    }
}
