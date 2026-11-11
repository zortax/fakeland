//! The context of one action: what injects its input, its cancellation token and a hook that
//! sees every injected event as it goes out.

use fakeland_trace::InputSource;

use crate::input::SentSink;
use crate::util::CancelToken;

/// A callback that sees the records of injected events right after their flush.
pub type SentHook = SentSink;

/// The context of one action.
#[derive(Clone)]
pub struct ActionCtx {
    /// What injects the input.
    pub source: InputSource,
    /// Stops the action and its timers.
    pub token: CancelToken,
    /// Sees injected events as they go out.
    pub on_sent: Option<SentHook>,
}

impl std::fmt::Debug for ActionCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionCtx")
            .field("source", &self.source)
            .field("cancelled", &self.token.is_cancelled())
            .finish()
    }
}

impl ActionCtx {
    /// Builds a context for a source with a fresh token.
    pub fn new(source: InputSource) -> Self {
        Self {
            source,
            token: CancelToken::new(),
            on_sent: None,
        }
    }

    /// Returns the context with a hook for injected events.
    pub fn with_sent_hook(mut self, hook: SentHook) -> Self {
        self.on_sent = Some(hook);
        self
    }

    /// Returns the context with another token.
    pub fn with_token(mut self, token: CancelToken) -> Self {
        self.token = token;
        self
    }
}
