//! Buffer and synchronization details of a commit.

use fakeland_types::{BufferId, Logical, Size};
use serde::{Deserialize, Serialize};

/// The memory type of a client buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferKind {
    /// A `wl_shm` buffer in shared memory.
    Shm,
    /// A `linux-dmabuf` buffer.
    Dmabuf,
    /// A `wp_single_pixel_buffer_v1` buffer.
    SinglePixel,
}

impl BufferKind {
    /// Returns the snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shm => "shm",
            Self::Dmabuf => "dmabuf",
            Self::SinglePixel => "single_pixel",
        }
    }
}

/// The GPU synchronization of a commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncKind {
    /// No fence. The buffer counts as ready at commit time.
    #[default]
    None,
    /// Implicit fences attached to the dmabuf.
    Implicit,
    /// An explicit `linux-drm-syncobj-v1` acquire point.
    Explicit,
}

impl SyncKind {
    /// Returns the snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Implicit => "implicit",
            Self::Explicit => "explicit",
        }
    }
}

/// The buffer that a commit attaches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BufferInfo {
    /// The buffer.
    pub id: BufferId,
    /// The memory type.
    pub kind: BufferKind,
    /// The DRM fourcc format code.
    pub format: u32,
    /// The DRM format modifier of a dmabuf.
    pub modifier: Option<u64>,
    /// The buffer width in pixels.
    pub width: i32,
    /// The buffer height in pixels.
    pub height: i32,
    /// The buffer scale: the integer `wl_surface` buffer scale or the fractional scale.
    pub scale: f64,
    /// The `wp_viewport` destination size.
    pub viewport: Option<Size<Logical>>,
}

impl BufferInfo {
    /// Returns the buffer area in pixels.
    pub const fn area(&self) -> i64 {
        self.width as i64 * self.height as i64
    }

    /// Returns the fourcc format as four ASCII characters, for example `XR24`.
    pub fn fourcc(&self) -> String {
        self.format
            .to_le_bytes()
            .iter()
            .map(|&b| if b.is_ascii_graphic() { b as char } else { '?' })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourcc_reads_little_endian() {
        let b = BufferInfo {
            id: BufferId(1),
            kind: BufferKind::Dmabuf,
            format: u32::from_le_bytes(*b"XR24"),
            modifier: None,
            width: 4,
            height: 3,
            scale: 1.0,
            viewport: None,
        };
        assert_eq!(b.fourcc(), "XR24");
        assert_eq!(b.area(), 12);
    }
}
