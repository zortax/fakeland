//! The marks of one surface. Every call sets state that applies on the next commit of the
//! surface.

use fakeland_protocol::client::fakeland_surface_marks_v1::FakelandSurfaceMarksV1;
use fakeland_protocol::wire::{WireSpan, split_u64};
use fakeland_types::{InputEventId, Timestamp};
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;

use crate::check::{self, MarkError};
use crate::claims::Claims;

/// A failure to create the marks of a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceError {
    /// The surface already has a live `SurfaceMarks`.
    #[error("the surface already has marks")]
    Taken,
    /// The wl_surface is destroyed.
    #[error("the surface is destroyed")]
    SurfaceDestroyed,
}

/// The marks of one surface.
///
/// Every call sets double-buffered state, and the compositor applies it on the next
/// wl_surface.commit of the surface. Calls on an inactive value, or after the wl_surface is
/// destroyed, do nothing. Dropping the value destroys the marks object and discards marks that
/// no commit applied.
#[derive(Debug)]
pub struct SurfaceMarks {
    active: Option<Active>,
}

#[derive(Debug)]
struct Active {
    marks: FakelandSurfaceMarksV1,
    surface: WlSurface,
    claims: Claims,
}

impl SurfaceMarks {
    pub(crate) fn inactive() -> Self {
        Self { active: None }
    }

    pub(crate) fn active(
        marks: FakelandSurfaceMarksV1,
        surface: WlSurface,
        claims: Claims,
    ) -> Self {
        Self {
            active: Some(Active {
                marks,
                surface,
                claims,
            }),
        }
    }

    /// Returns true when the marks reach a compositor.
    pub fn is_active(&self) -> bool {
        self.live().is_some()
    }

    /// Sets the app frame id of the next commit. A second call before the commit replaces the
    /// first one.
    pub fn frame(&self, id: u64) {
        if let Some(marks) = self.live() {
            let (hi, lo) = split_u64(id);
            marks.frame(hi, lo);
        }
    }

    /// States that the next commit contains the visible effect of an input event. Call it once
    /// per input event that the commit contains.
    pub fn input_handled(&self, id: InputEventId) {
        if let Some(marks) = self.live() {
            let (hi, lo) = split_u64(id.0);
            marks.input_handled(hi, lo);
        }
    }

    /// Attaches a client-side phase with `CLOCK_MONOTONIC` begin and end times to the next
    /// commit.
    ///
    /// The call checks the name and the order of the times also on an inactive value.
    pub fn span(&self, name: &str, begin: Timestamp, end: Timestamp) -> Result<(), MarkError> {
        check::span(name, begin, end)?;
        if let Some(marks) = self.live() {
            let WireSpan { begin, end } = WireSpan::new(begin, end);
            marks.span(
                name.to_owned(),
                begin.sec_hi,
                begin.sec_lo,
                begin.nsec,
                end.sec_hi,
                end.sec_lo,
                end.nsec,
            );
        }
        Ok(())
    }

    /// Attaches a named mark to the next commit.
    ///
    /// The call checks the name also on an inactive value.
    pub fn mark(&self, name: &str) -> Result<(), MarkError> {
        check::mark(name)?;
        if let Some(marks) = self.live() {
            marks.mark(name.to_owned());
        }
        Ok(())
    }

    /// Returns the marks object while its wl_surface lives.
    fn live(&self) -> Option<&FakelandSurfaceMarksV1> {
        self.active
            .as_ref()
            .filter(|a| a.surface.is_alive())
            .map(|a| &a.marks)
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        self.marks.destroy();
        self.claims.release(&self.surface.id());
    }
}
