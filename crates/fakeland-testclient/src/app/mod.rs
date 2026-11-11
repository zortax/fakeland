//! The client: its state, the event loop, window management, input handling, frame rendering
//! and compositor feedback.

mod anim_clock;
mod configure;
mod damage;
mod exit;
mod feedback;
mod frame;
mod inputs;
mod run;
mod state;
mod tick;
mod timer;
mod window;

pub use anim_clock::{AnimClock, AnimInputs};
pub use configure::{ConfigureQueue, PendingConfigure, decode_states};
pub use damage::{DamageMode, Previous, frame_damage};
pub use exit::{ExitReason, USAGE_EXIT_CODE};
pub use run::run;
pub use state::{App, ToplevelConfigure};
pub use timer::Timer;
