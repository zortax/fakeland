//! The renderer of a session: GLES on a render node, or Pixman on the CPU. It imports client
//! buffers, builds the dmabuf feedback and names the device for the trace header.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Format, Fourcc};
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{ImportDma, ImportMemWl};
use smithay::reexports::wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1::TrancheFlags;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder};

use super::{GlesDevice, RenderError};
use crate::config::RendererChoice;
use crate::recording::DeviceFacts;

/// Formats that a scanout plane takes.
const SCANOUT_FORMATS: &[Fourcc] = &[
    Fourcc::Xrgb8888,
    Fourcc::Argb8888,
    Fourcc::Xbgr8888,
    Fourcc::Abgr8888,
    Fourcc::Xrgb2101010,
    Fourcc::Xbgr2101010,
];

/// The session renderer.
pub enum RenderBackend {
    /// GLES on a render node.
    Gles(Box<GlesDevice>),
    /// Pixman on the CPU.
    Pixman(PixmanRenderer),
}

impl std::fmt::Debug for RenderBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gles(d) => d.fmt(f),
            Self::Pixman(_) => f.write_str("Pixman"),
        }
    }
}

impl RenderBackend {
    /// Opens the renderer that the config selects.
    pub fn open(
        choice: RendererChoice,
        node: Option<&std::path::Path>,
    ) -> Result<Self, RenderError> {
        let pixman = || {
            PixmanRenderer::new()
                .map(Self::Pixman)
                .map_err(|e| RenderError::Init(e.to_string()))
        };
        match choice {
            RendererChoice::Pixman => pixman(),
            RendererChoice::Gles => GlesDevice::open(node).map(|d| Self::Gles(Box::new(d))),
            RendererChoice::Auto => match GlesDevice::open(node) {
                Ok(d) => Ok(Self::Gles(Box::new(d))),
                Err(e) => {
                    tracing::warn!("GLES renderer unavailable, using Pixman: {e}");
                    pixman()
                }
            },
        }
    }

    /// Returns the renderer name for the trace header.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gles(_) => "gles",
            Self::Pixman(_) => "pixman",
        }
    }

    /// Returns the device facts for the trace header.
    pub fn facts(&self) -> DeviceFacts {
        match self {
            Self::Gles(d) => DeviceFacts {
                node: Some(d.path.display().to_string()),
                driver: (!d.driver.is_empty()).then(|| d.driver.clone()),
                renderer: self.name().into(),
            },
            Self::Pixman(_) => DeviceFacts {
                node: None,
                driver: None,
                renderer: self.name().into(),
            },
        }
    }

    /// Returns the shm formats the renderer imports beyond ARGB8888 and XRGB8888.
    pub fn shm_formats(&mut self) -> Vec<wl_shm::Format> {
        let all: Vec<wl_shm::Format> = match self {
            Self::Gles(d) => d
                .renderer()
                .map(|r| r.shm_formats().collect())
                .unwrap_or_default(),
            Self::Pixman(r) => r.shm_formats().collect(),
        };
        all.into_iter()
            .filter(|f| !matches!(f, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888))
            .collect()
    }

    /// Returns the default dmabuf feedback: the render node as main device with one tranche of
    /// every importable format.
    pub fn default_feedback(&self) -> Option<DmabufFeedback> {
        let Self::Gles(d) = self else { return None };
        DmabufFeedbackBuilder::new(d.node.dev_id(), d.formats.iter().copied())
            .build()
            .map_err(|e| tracing::warn!("dmabuf feedback failed: {e}"))
            .ok()
    }

    /// Returns the feedback of a fullscreen surface: a scanout tranche of the plane formats
    /// before the render tranche.
    pub fn scanout_feedback(&self) -> Option<DmabufFeedback> {
        let Self::Gles(d) = self else { return None };
        let scanout: Vec<Format> = d
            .formats
            .iter()
            .copied()
            .filter(|f| SCANOUT_FORMATS.contains(&f.code))
            .collect();
        DmabufFeedbackBuilder::new(d.node.dev_id(), d.formats.iter().copied())
            .add_preference_tranche(d.node.dev_id(), Some(TrancheFlags::Scanout), scanout)
            .build()
            .map_err(|e| tracing::warn!("scanout feedback failed: {e}"))
            .ok()
    }

    /// Returns the device for syncobj imports when the device supports `syncobj_eventfd`.
    pub fn syncobj_device(&self) -> Option<DrmDeviceFd> {
        match self {
            Self::Gles(d) if d.syncobj => Some(d.device_fd.clone()),
            _ => None,
        }
    }

    /// Imports a dmabuf to check that the renderer can sample it.
    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        match self {
            Self::Gles(d) => d
                .renderer()
                .is_some_and(|r| r.import_dmabuf(dmabuf, None).is_ok()),
            Self::Pixman(r) => r.import_dmabuf(dmabuf, None).is_ok(),
        }
    }
}
