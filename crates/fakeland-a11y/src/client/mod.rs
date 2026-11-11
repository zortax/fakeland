//! The AT-SPI client: connection, app listing, tree walks, search and node requests.

mod act;
mod apps;
mod blocking;
mod connect;
mod context;
mod fetch;
mod proxy;
#[cfg(test)]
mod send;
mod tree;
mod walk;

pub use apps::{AppInfo, FrameInfo};
pub use connect::{A11y, BUS_NAME, BusStatus, DEFAULT_CALL_TIMEOUT, bus_address, bus_status};
