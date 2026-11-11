//! Subscriptions to trace records.

use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::method;

/// A subscription request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscribeParams {
    /// The record types to send, such as `presented` or `window_mapped`. Empty sends every type.
    #[serde(default)]
    pub types: Vec<String>,
    /// The queue length. A full queue drops records and sends a gap event.
    #[serde(default = "defaults::subscription_capacity")]
    pub capacity: usize,
}

impl Default for SubscribeParams {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            capacity: defaults::subscription_capacity(),
        }
    }
}

/// A started subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    /// The subscription id of the events.
    pub subscription: u64,
}

/// A subscription to end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsubscribeParams {
    /// The subscription id.
    pub subscription: u64,
}

method!(
    /// Streams trace records as events.
    Subscribe, "subscribe", SubscribeParams => SubscriptionInfo
);
method!(
    /// Ends a subscription. Returns false for an unknown id.
    Unsubscribe, "unsubscribe", UnsubscribeParams => bool
);
