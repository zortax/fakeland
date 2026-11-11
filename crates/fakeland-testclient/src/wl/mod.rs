//! The Wayland side of the client: global binding, the surface objects, the shm buffer pool,
//! scale handling, the keymap, and one event handler per protocol.

mod commit_timing;
mod compositor;
mod fifo;
mod fractional;
mod globals;
mod keyboard;
mod keymap;
mod output;
mod pointer;
mod pool;
mod presentation;
mod registry;
mod scale;
mod seat;
mod shm;
mod surface;
mod text_input;
mod touch;
mod viewporter;
mod xdg;

pub use commit_timing::timer_args;
pub use compositor::FrameTag;
pub use globals::{Globals, MissingGlobal};
pub use keymap::{KeyState, keysym_name};
pub use output::Outputs;
pub use pool::{BufferPool, BufferTag, PoolError};
pub use presentation::{FeedbackTag, Presentation};
pub use scale::{Scale, ScaleMode, ScaleState, Strategy};
pub use seat::SeatState;
pub use surface::{SurfaceObjects, SurfaceSetup};
