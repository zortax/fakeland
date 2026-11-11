//! The reactor trait and the closure adapter.

use std::marker::PhantomData;

use fakeland_trace::Record;

use super::ReactorCx;
use crate::framecap::CapturedFrame;

/// Code that runs on the compositor thread and reacts to trace records.
///
/// Every callback runs on the compositor thread right after the dispatch that produced the
/// record, with full access to the compositor through [`ReactorCx`]. A callback must return
/// quickly: it runs inside the event loop, and its time adds to compositor lateness.
pub trait Reactor: Send + 'static {
    /// The values the reactor sends to its handle.
    type Value: Send + 'static;

    /// Runs once when the reactor is installed.
    fn on_start(&mut self, cx: &mut ReactorCx<'_, Self::Value>) {
        let _ = cx;
    }

    /// Runs for every trace record, in record order.
    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, Self::Value>);

    /// Runs with a frame that [`ReactorCx::capture_next_commit`] asked for.
    fn on_capture(&mut self, frame: CapturedFrame, cx: &mut ReactorCx<'_, Self::Value>) {
        let _ = (frame, cx);
    }

    /// Runs at a time that [`ReactorCx::wake_at`] asked for, with its token.
    fn on_timer(&mut self, token: u64, cx: &mut ReactorCx<'_, Self::Value>) {
        let _ = (token, cx);
    }
}

impl<R: Reactor + ?Sized> Reactor for Box<R> {
    type Value = R::Value;

    fn on_start(&mut self, cx: &mut ReactorCx<'_, Self::Value>) {
        (**self).on_start(cx);
    }

    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, Self::Value>) {
        (**self).on_record(record, cx);
    }

    fn on_capture(&mut self, frame: CapturedFrame, cx: &mut ReactorCx<'_, Self::Value>) {
        (**self).on_capture(frame, cx);
    }

    fn on_timer(&mut self, token: u64, cx: &mut ReactorCx<'_, Self::Value>) {
        (**self).on_timer(token, cx);
    }
}

/// A reactor made from a closure over records.
pub struct FnReactor<V, F> {
    f: F,
    value: PhantomData<fn() -> V>,
}

impl<V, F> Reactor for FnReactor<V, F>
where
    V: Send + 'static,
    F: FnMut(&Record, &mut ReactorCx<'_, V>) + Send + 'static,
{
    type Value = V;

    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, V>) {
        (self.f)(record, cx);
    }
}

/// Builds a reactor from a closure that sees every record.
pub fn reactor_fn<V, F>(f: F) -> FnReactor<V, F>
where
    V: Send + 'static,
    F: FnMut(&Record, &mut ReactorCx<'_, V>) + Send + 'static,
{
    FnReactor {
        f,
        value: PhantomData,
    }
}
