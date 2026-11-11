//! Errors of renderer start.

use std::path::PathBuf;

use thiserror::Error;

/// A renderer that cannot start.
#[derive(Debug, Error)]
pub enum RenderError {
    /// No render node exists.
    #[error("no DRM render node found")]
    NoRenderNode,
    /// The render node does not open.
    #[error("cannot open {path}: {source}")]
    Open {
        /// The node path.
        path: PathBuf,
        /// The open error.
        source: std::io::Error,
    },
    /// GBM, EGL or the renderer fails to initialize.
    #[error("renderer init failed: {0}")]
    Init(String),
}
