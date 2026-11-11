//! What a frame capture records and what it returns.

use fakeland_scenario::WindowRef;
use fakeland_types::{CommitId, Logical, Nanos, Rect, SurfaceId, Timestamp, WindowId};
use fakeland_vision::Image;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn one() -> u32 {
    1
}

fn default_max_frames() -> usize {
    600
}

/// When a frame capture takes an image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaptureSpec {
    /// After smithay applies each commit of the window tree, which happens at its latch.
    EveryCommit {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// A rectangle in window-relative logical coordinates. `None` covers the window.
        #[serde(default)]
        roi: Option<Rect<Logical>>,
        /// An integer box-filter factor. 1 keeps the output resolution.
        #[serde(default = "one")]
        downscale: u32,
        /// The capture stops after this many frames and counts the rest as dropped.
        #[serde(default = "default_max_frames")]
        max_frames: usize,
    },
    /// At each presentation of a commit of the window tree.
    EveryPresented {
        /// The window.
        #[serde(default)]
        window: WindowRef,
        /// A rectangle in window-relative logical coordinates. `None` covers the window.
        #[serde(default)]
        roi: Option<Rect<Logical>>,
        /// An integer box-filter factor. 1 keeps the output resolution.
        #[serde(default = "one")]
        downscale: u32,
        /// The capture stops after this many frames and counts the rest as dropped.
        #[serde(default = "default_max_frames")]
        max_frames: usize,
    },
}

impl CaptureSpec {
    /// Captures every applied commit of a window at full resolution.
    pub fn every_commit(window: impl Into<WindowRef>) -> Self {
        Self::EveryCommit {
            window: window.into(),
            roi: None,
            downscale: 1,
            max_frames: default_max_frames(),
        }
    }

    /// Captures every presented frame of a window at full resolution.
    pub fn every_presented(window: impl Into<WindowRef>) -> Self {
        Self::EveryPresented {
            window: window.into(),
            roi: None,
            downscale: 1,
            max_frames: default_max_frames(),
        }
    }

    /// Returns the spec with a region.
    pub fn with_roi(mut self, rect: Rect<Logical>) -> Self {
        match &mut self {
            Self::EveryCommit { roi, .. } | Self::EveryPresented { roi, .. } => *roi = Some(rect),
        }
        self
    }

    /// Returns the spec with a downscale factor.
    pub fn with_downscale(mut self, factor: u32) -> Self {
        match &mut self {
            Self::EveryCommit { downscale, .. } | Self::EveryPresented { downscale, .. } => {
                *downscale = factor.max(1)
            }
        }
        self
    }

    /// Returns the spec with a frame limit.
    pub fn with_max_frames(mut self, n: usize) -> Self {
        match &mut self {
            Self::EveryCommit { max_frames, .. } | Self::EveryPresented { max_frames, .. } => {
                *max_frames = n
            }
        }
        self
    }

    /// Returns the window.
    pub fn window(&self) -> &WindowRef {
        match self {
            Self::EveryCommit { window, .. } | Self::EveryPresented { window, .. } => window,
        }
    }

    /// Returns the region.
    pub fn roi(&self) -> Option<Rect<Logical>> {
        match self {
            Self::EveryCommit { roi, .. } | Self::EveryPresented { roi, .. } => *roi,
        }
    }

    /// Returns the downscale factor, at least 1.
    pub fn downscale(&self) -> u32 {
        match self {
            Self::EveryCommit { downscale, .. } | Self::EveryPresented { downscale, .. } => {
                (*downscale).max(1)
            }
        }
    }

    /// Returns the frame limit.
    pub fn max_frames(&self) -> usize {
        match self {
            Self::EveryCommit { max_frames, .. } | Self::EveryPresented { max_frames, .. } => {
                *max_frames
            }
        }
    }
}

/// One captured frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CapturedFrame {
    /// The commit that the image shows.
    pub commit: CommitId,
    /// The surface of the commit.
    pub surface: SurfaceId,
    /// The window.
    pub window: WindowId,
    /// The capture time.
    pub time: Timestamp,
    /// The modeled presentation time, for presented frames.
    pub presented: Option<Timestamp>,
    /// The vblank sequence, for presented frames.
    pub seq: Option<u64>,
    /// The pixels, RGBA, top row first.
    pub image: Image,
    /// The captured rectangle in the global logical space.
    pub region: Rect<Logical>,
    /// Logical units per image pixel.
    pub scale: f64,
}

/// The frames of one capture.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameCapture {
    /// What the capture recorded.
    pub spec: CaptureSpec,
    /// The frames in capture order.
    pub frames: Vec<CapturedFrame>,
    /// Frames past the limit or that failed to render.
    pub dropped: u64,
    /// The compositor time spent on the captures.
    pub cost: Nanos,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_defaults_and_builders() {
        let s: CaptureSpec = serde_json::from_str(r#"{"type":"every_presented"}"#).unwrap();
        assert_eq!(s.window(), &WindowRef::Focused);
        assert_eq!(s.downscale(), 1);
        assert_eq!(s.max_frames(), 600);
        let s = CaptureSpec::every_commit(WindowRef::Any)
            .with_roi(Rect::new(0, 0, 10, 10))
            .with_downscale(0)
            .with_max_frames(5);
        assert_eq!(s.downscale(), 1);
        assert_eq!(s.max_frames(), 5);
        assert_eq!(s.roi(), Some(Rect::new(0, 0, 10, 10)));
        let back: CaptureSpec = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
