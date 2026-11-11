//! Client-side checks of mark names and spans. The compositor raises a protocol error for an
//! invalid name or a reversed span, so the crate rejects both before it sends a request.

use fakeland_protocol::wire::{NameError, check_name};
use fakeland_types::Timestamp;

/// A mark or span that the protocol rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MarkError {
    /// The name is empty or too long.
    #[error("invalid mark name: {0}")]
    Name(#[from] NameError),
    /// The span ends before it begins.
    #[error("the span ends at {end} before it begins at {begin}")]
    ReversedSpan {
        /// The begin time.
        begin: Timestamp,
        /// The end time.
        end: Timestamp,
    },
}

/// Checks a mark name.
pub(crate) fn mark(name: &str) -> Result<(), MarkError> {
    Ok(check_name(name)?)
}

/// Checks a span name and its order.
pub(crate) fn span(name: &str, begin: Timestamp, end: Timestamp) -> Result<(), MarkError> {
    check_name(name)?;
    if end < begin {
        return Err(MarkError::ReversedSpan { begin, end });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_marks_and_spans() {
        assert_eq!(mark("ready"), Ok(()));
        assert_eq!(span("paint", Timestamp(5), Timestamp(5)), Ok(()));
        assert_eq!(span("paint", Timestamp(5), Timestamp(9)), Ok(()));
    }

    #[test]
    fn rejects_bad_names() {
        assert_eq!(mark(""), Err(MarkError::Name(NameError::Empty)));
        let long = "n".repeat(fakeland_protocol::wire::MAX_NAME_LEN + 1);
        assert!(matches!(
            span(&long, Timestamp(0), Timestamp(1)),
            Err(MarkError::Name(NameError::TooLong(_)))
        ));
    }

    #[test]
    fn rejects_reversed_spans() {
        assert_eq!(
            span("layout", Timestamp(9), Timestamp(5)),
            Err(MarkError::ReversedSpan {
                begin: Timestamp(9),
                end: Timestamp(5)
            })
        );
    }
}
