//! wp_presentation events: the clock id, and the presented or discarded feedback of each frame.

use fakeland_types::Timestamp;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::wp::presentation_time::client::wp_presentation::{self, WpPresentation};
use wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::{
    self, WpPresentationFeedback,
};

use crate::app::App;

/// The user data of a feedback object: the frame it reports on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedbackTag(pub u64);

/// A decoded wp_presentation_feedback.presented event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    /// The presentation time.
    pub time: Timestamp,
    /// The refresh period in nanoseconds; 0 when unknown.
    pub refresh: u32,
    /// The vblank sequence.
    pub seq: u64,
    /// The kind flags.
    pub flags: u32,
}

impl Dispatch<WpPresentation, ()> for App {
    fn event(
        app: &mut Self,
        _: &WpPresentation,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            app.presentation_clock(clk_id);
        }
    }
}

impl Dispatch<WpPresentationFeedback, FeedbackTag> for App {
    fn event(
        app: &mut Self,
        _: &WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        tag: &FeedbackTag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                seq_hi,
                seq_lo,
                flags,
            } => {
                let secs = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
                let flags = match flags {
                    WEnum::Value(k) => k.bits(),
                    WEnum::Unknown(bits) => bits,
                };
                app.presented(
                    tag.0,
                    Presentation {
                        time: Timestamp::from_secs_nanos(secs, tv_nsec),
                        refresh,
                        seq: ((seq_hi as u64) << 32) | seq_lo as u64,
                        flags,
                    },
                );
            }
            wp_presentation_feedback::Event::Discarded => app.discarded(tag.0),
            _ => {}
        }
    }
}
