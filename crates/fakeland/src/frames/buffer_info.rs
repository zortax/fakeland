//! Buffer descriptions for the trace: session buffer ids, kind, fourcc, modifier and size of a
//! `wl_buffer`.

use std::collections::HashMap;

use fakeland_trace::{BufferInfo, BufferKind};
use fakeland_types::{BufferId, Logical, Size};
use smithay::backend::allocator::Buffer as _;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::backend::ObjectId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::{shm_format_to_fourcc, with_buffer_contents};
use smithay::wayland::single_pixel_buffer::get_single_pixel_buffer;

use crate::server::Ids;

/// The DRM format modifier for an invalid modifier.
const DRM_FORMAT_MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

/// Session ids of live buffers.
#[derive(Debug, Default)]
pub struct BufferIds {
    ids: HashMap<ObjectId, BufferId>,
}

impl BufferIds {
    /// Returns the id of a buffer, allocating one on first sight.
    pub fn id(&mut self, buffer: &WlBuffer, ids: &mut Ids) -> BufferId {
        *self.ids.entry(buffer.id()).or_insert_with(|| ids.buffer())
    }

    /// Forgets a destroyed buffer.
    pub fn forget(&mut self, buffer: &WlBuffer) {
        self.ids.remove(&buffer.id());
    }
}

/// Converts a `wl_shm` format to a DRM fourcc code. The two legacy formats have their own
/// values; every other `wl_shm` format equals its fourcc.
pub fn shm_fourcc(format: wl_shm::Format) -> u32 {
    shm_format_to_fourcc(format)
        .map(|f| f as u32)
        .unwrap_or_else(|| u32::from(format))
}

/// Describes a buffer. `None` means the buffer kind is unknown.
pub fn describe(
    buffer: &WlBuffer,
    id: BufferId,
    scale: i32,
    viewport: Option<Size<Logical>>,
) -> Option<BufferInfo> {
    let base = |kind, format, modifier, width, height| BufferInfo {
        id,
        kind,
        format,
        modifier,
        width,
        height,
        scale: scale as f64,
        viewport,
    };
    if let Ok(dmabuf) = get_dmabuf(buffer) {
        let format = dmabuf.format();
        let modifier = u64::from(format.modifier);
        let size = dmabuf.size();
        return Some(base(
            BufferKind::Dmabuf,
            format.code as u32,
            (modifier != DRM_FORMAT_MOD_INVALID).then_some(modifier),
            size.w,
            size.h,
        ));
    }
    if get_single_pixel_buffer(buffer).is_ok() {
        return Some(base(
            BufferKind::SinglePixel,
            smithay::backend::allocator::Fourcc::Argb8888 as u32,
            None,
            1,
            1,
        ));
    }
    with_buffer_contents(buffer, |_, _, data| {
        base(
            BufferKind::Shm,
            shm_fourcc(data.format),
            None,
            data.width,
            data.height,
        )
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_shm_formats_map_to_fourcc() {
        assert_eq!(
            shm_fourcc(wl_shm::Format::Argb8888),
            u32::from_le_bytes(*b"AR24")
        );
        assert_eq!(
            shm_fourcc(wl_shm::Format::Xrgb8888),
            u32::from_le_bytes(*b"XR24")
        );
        assert_eq!(
            shm_fourcc(wl_shm::Format::Abgr8888),
            u32::from_le_bytes(*b"AB24")
        );
    }
}
