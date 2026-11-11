//! The present mode that a client's commits suggest.

use serde::{Deserialize, Serialize};

/// The present mode of a client, inferred from its commits and presentations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentMode {
    /// The client uses fifo barriers.
    Fifo,
    /// The client replaces queued frames: more than 5% of its commits are discarded.
    Mailbox,
    /// Each frame waits for the display, paced by frame callbacks or presentation feedback.
    Vsync,
    /// Presentations carry no vsync flag.
    Immediate,
    /// Too few frames to tell.
    Unknown,
}

/// The discard fraction above which a client counts as mailbox.
const MAILBOX_DISCARDS: f64 = 0.05;

/// Infers the present mode from frame counts and flags.
pub fn infer(
    commits: usize,
    discarded: usize,
    fifo: usize,
    presented: usize,
    without_vsync: usize,
) -> PresentMode {
    if commits < 2 || presented == 0 {
        return PresentMode::Unknown;
    }
    if fifo * 2 > commits {
        PresentMode::Fifo
    } else if discarded as f64 > MAILBOX_DISCARDS * commits as f64 {
        PresentMode::Mailbox
    } else if without_vsync * 2 > presented {
        PresentMode::Immediate
    } else {
        PresentMode::Vsync
    }
}
