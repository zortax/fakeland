//! The viewer window as a client of the host compositor. It runs on its own thread with its own
//! connection, opens one toplevel per output, attaches each composited dmabuf to a host surface
//! as it is, scales it with a viewport, and turns host input into output coordinates. Without
//! dmabuf import on the host, frames arrive as pixels and go through shared memory.

mod app;
mod buffers;
mod connect;
mod dispatch;
mod keymap;
mod run;
mod seat;
mod shm;
mod window;

pub use run::spawn;
