//! The GLES renderer on a DRM render node: node discovery, GBM, EGL through the smithay GPU
//! manager, importable dmabuf formats and explicit sync support.

use std::fs::{self, OpenOptions};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use smithay::backend::allocator::Format;
use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::drm::{DrmDeviceFd, DrmNode};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::multigpu::{ApiDevice, GpuManager};
use smithay::reexports::drm::Device as _;
use smithay::utils::DeviceFd;
use smithay::wayland::drm_syncobj::supports_syncobj_eventfd;

use super::RenderError;

/// The default render node.
const DEFAULT_NODE: &str = "/dev/dri/renderD128";

/// The GPU manager type of the GLES backend.
pub type GlesGpu = GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>;

/// A GLES renderer on one render node.
pub struct GlesDevice {
    gpu: GlesGpu,
    /// The render node.
    pub node: DrmNode,
    /// The node path.
    pub path: PathBuf,
    /// The device fd for syncobj imports.
    pub device_fd: DrmDeviceFd,
    /// The dmabuf formats the renderer imports.
    pub formats: Vec<Format>,
    /// The device supports `syncobj_eventfd`.
    pub syncobj: bool,
    /// The DRM driver name.
    pub driver: String,
}

impl std::fmt::Debug for GlesDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlesDevice")
            .field("path", &self.path)
            .field("driver", &self.driver)
            .field("syncobj", &self.syncobj)
            .field("formats", &self.formats.len())
            .finish()
    }
}

impl GlesDevice {
    /// Opens a render node. `None` selects the default node or the first render node.
    pub fn open(path: Option<&Path>) -> Result<Self, RenderError> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => find_render_node().ok_or(RenderError::NoRenderNode)?,
        };
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| RenderError::Open {
                path: path.clone(),
                source,
            })?;
        let fd: OwnedFd = file.into();
        let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));
        let node = DrmNode::from_file(&device_fd).map_err(|e| RenderError::Init(e.to_string()))?;
        let driver = device_fd
            .get_driver()
            .map(|d| d.name().to_string_lossy().into_owned())
            .unwrap_or_default();
        let gbm =
            GbmDevice::new(device_fd.clone()).map_err(|e| RenderError::Init(e.to_string()))?;
        let mut gpu = GpuManager::new(GbmGlesBackend::default())
            .map_err(|e| RenderError::Init(e.to_string()))?;
        gpu.as_mut()
            .add_node(node, gbm)
            .map_err(|e| RenderError::Init(e.to_string()))?;
        let syncobj = supports_syncobj_eventfd(&device_fd);
        let mut dev = Self {
            gpu,
            node,
            path,
            device_fd,
            formats: Vec::new(),
            syncobj,
            driver,
        };
        let formats: Vec<Format> = dev
            .renderer()
            .ok_or_else(|| RenderError::Init("the GPU manager has no renderer".into()))?
            .dmabuf_formats()
            .into_iter()
            .collect();
        dev.formats = formats;
        Ok(dev)
    }

    /// Returns the renderer.
    pub fn renderer(&mut self) -> Option<&mut GlesRenderer> {
        self.gpu
            .devices_mut()
            .ok()?
            .next()
            .map(|d| d.renderer_mut())
    }
}

/// Returns the default render node, or the first render node in `/dev/dri`.
pub fn find_render_node() -> Option<PathBuf> {
    let default = Path::new(DEFAULT_NODE);
    if default.exists() {
        return Some(default.to_path_buf());
    }
    let mut nodes: Vec<PathBuf> = fs::read_dir("/dev/dri")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("renderD"))
        })
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}
