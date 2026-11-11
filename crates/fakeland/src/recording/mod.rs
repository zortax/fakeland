//! Trace recording on the compositor thread: the event bus with its recorder, live subscriptions
//! and the host environment of the trace header.

mod bus;
mod environment;
mod subscription;

pub use bus::EventBus;
pub use environment::{DeviceFacts, detect};
pub use subscription::{Subscriber, Subscription, SubscriptionItem};
