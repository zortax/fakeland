//! Reactors for benches: compositor-side drivers that react to trace records without a round
//! trip, such as a ping-pong latency loop.

mod analyzer;
mod ping_pong;

pub use analyzer::{PingPongLatency, PingPongReport};
pub use ping_pong::{PingPong, PingPongSample};
