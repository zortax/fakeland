//! A fake-clock simulation harness: synthetic clients drive a scheduler through submit and advance.

#![allow(dead_code, unused_imports)]

mod clients;
mod sim;

pub use clients::{Client, FifoClient, Mailbox, PacedClient, Renderer};
pub use sim::{P60, START, Sim, ms, t60, us};
