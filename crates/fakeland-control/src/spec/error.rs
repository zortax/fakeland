//! Errors of setting text forms.

use thiserror::Error;

/// A setting that does not parse or does not validate.
#[derive(Debug, Error)]
pub enum SpecError {
    /// An output form is invalid.
    #[error("invalid output {0:?}: use WIDTHxHEIGHT[@HZ], optionally with NAME: in front")]
    Output(String),
    /// A size form is invalid.
    #[error("invalid size {0:?}: use WIDTHxHEIGHT")]
    Size(String),
    /// A timing form is invalid.
    #[error("invalid timing {0:?}: use fixed, vrr:MIN-MAX, unthrottled, manual or manual:HZ")]
    Timing(String),
    /// A callback form is invalid.
    #[error("invalid callbacks {0:?}: use at-vblank, before-vblank:LEAD or on-present")]
    Callbacks(String),
    /// A renderer name is invalid.
    #[error("invalid renderer {0:?}: use auto, gles or pixman")]
    Renderer(String),
    /// A composite mode is invalid.
    #[error("invalid composite mode {0:?}: use lazy or every-frame")]
    Composite(String),
    /// A window policy is invalid.
    #[error("invalid window policy {0:?}: use kiosk, floating[:WxH] or fullscreen")]
    WindowPolicy(String),
    /// A window matcher is invalid.
    #[error("invalid window {0:?}: use focused, any, ID, id:ID, app:APP_ID or title:REGEX")]
    Window(String),
    /// A duration is invalid.
    #[error("invalid duration {0:?}")]
    Duration(String),
    /// The resulting config is invalid.
    #[error(transparent)]
    Config(#[from] fakeland::ConfigError),
}
