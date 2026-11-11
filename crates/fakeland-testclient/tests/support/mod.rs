//! A minimal in-process compositor that drives fakeland-testclient: wl_compositor, wl_shm,
//! xdg-shell, a pointer seat and fakeland_marks_v1.

mod compositor;
mod marks;
mod seat;
mod server;
mod shm;
mod state;
mod timing;
mod xdg;

pub use server::TestCompositor;
pub use state::Commit;
