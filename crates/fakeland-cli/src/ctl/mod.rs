//! `fakeland ctl`: drive a running session through its control socket.

mod a11y;
mod args;
mod bench;
mod exec;
mod input;
mod media;
mod position;
mod print;
mod session;
mod trace;

pub use args::CtlArgs;
pub use exec::run;
