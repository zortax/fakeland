//! A span guard. It takes the begin time on creation and the end time on finish, both from
//! `CLOCK_MONOTONIC`, and records the span on the marks of a surface.

use std::borrow::Cow;

use fakeland_types::{Clock, MonotonicClock, Timestamp};

use crate::check::MarkError;
use crate::surface::SurfaceMarks;

/// A client-side phase that has begun.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a span records nothing until it finishes"]
pub struct Span {
    name: Cow<'static, str>,
    begin: Timestamp,
}

impl Span {
    /// Begins a span now.
    pub fn start(name: impl Into<Cow<'static, str>>) -> Self {
        Self::start_at(name, MonotonicClock.now())
    }

    /// Begins a span at a given time.
    pub fn start_at(name: impl Into<Cow<'static, str>>, begin: Timestamp) -> Self {
        Self {
            name: name.into(),
            begin,
        }
    }

    /// Returns the name of the span.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the begin time.
    pub fn began_at(&self) -> Timestamp {
        self.begin
    }

    /// Ends the span now and attaches it to the next commit of a surface.
    pub fn finish(self, target: &SurfaceMarks) -> Result<(), MarkError> {
        self.finish_at(target, MonotonicClock.now())
    }

    /// Ends the span at a given time and attaches it to the next commit of a surface.
    pub fn finish_at(self, target: &SurfaceMarks, end: Timestamp) -> Result<(), MarkError> {
        target.span(&self.name, self.begin, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_reads_the_monotonic_clock() {
        let before = MonotonicClock.now();
        let span = Span::start("layout");
        assert!(span.began_at() >= before);
        assert_eq!(span.name(), "layout");
    }

    #[test]
    fn finish_on_inactive_marks_checks_the_span() {
        let marks = SurfaceMarks::inactive();
        assert_eq!(Span::start("paint").finish(&marks), Ok(()));
        let reversed = Span::start_at("paint", Timestamp(10)).finish_at(&marks, Timestamp(5));
        assert!(matches!(reversed, Err(MarkError::ReversedSpan { .. })));
        assert!(Span::start(String::new()).finish(&marks).is_err());
    }
}
