//! Launching and isolating apps: the child environment, the private D-Bus bus, process groups,
//! captured output, exit reports and the client-to-process mapping.

mod a11y_bus;
mod a11y_env;
mod a11y_sweep;
mod app;
mod dbus;
mod deathsig;
mod env;
mod processes;
mod proctree;
mod ring;
mod spawn;
mod spec;

pub use a11y_bus::{A11yBus, start_a11y_bus};
pub use app::{App, AppShared};
pub use dbus::{DbusDaemon, DbusError};
pub use env::ChildEnv;
pub use processes::{ExitMsg, Processes};
pub use ring::RingBuffer;
pub use spec::{ExitInfo, LaunchError, LaunchSpec};
