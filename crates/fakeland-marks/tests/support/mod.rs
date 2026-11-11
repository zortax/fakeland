//! An in-process test compositor and a test app. The compositor implements wl_compositor,
//! wl_seat with a pointer and, on request, the fakeland_marks_v1 global. It reports what it
//! receives as records over a channel.

pub mod app;
mod compositor;
mod marks;
pub mod record;
mod seat;
pub mod server;
mod state;
